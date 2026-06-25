use mago_span::HasSpan;
use mago_syntax::cst::{ClassLikeMember, MethodBody, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0028";
static DESCRIPTION: &str = "Data Class";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_getter_setter_ratio: f64,
    pub min_methods: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_getter_setter_ratio: 0.7,
            min_methods: 3,
        }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl Rule {
    fn check_members(
        &self,
        file: &File<'_>,
        name: &str,
        members: &mago_syntax::cst::Sequence<'_, ClassLikeMember<'_>>,
        span: mago_span::Span,
        violations: &mut Vec<Violation>,
    ) {
        let mut field_count = 0;
        let mut total_methods = 0;
        let mut accessor_count = 0;
        let mut has_constructor = false;

        for member in members.iter() {
            match member {
                ClassLikeMember::Property(_) => field_count += 1,
                ClassLikeMember::Method(method) => {
                    total_methods += 1;
                    let name = std::str::from_utf8(method.name.value).unwrap_or_default();
                    if name == "__construct" {
                        has_constructor = true;
                        continue;
                    }
                    if is_accessor_method(method, name) {
                        accessor_count += 1;
                    }
                }
                _ => {}
            }
        }

        if field_count == 0 || total_methods < self.settings.min_methods {
            return;
        }

        let real_methods = total_methods - accessor_count - if has_constructor { 1 } else { 0 };

        if real_methods == 0 && total_methods >= self.settings.min_methods {
            let message = Message::new(
                "E0028:data-class",
                "\"{name}\" is a Data Class: {field_count} fields but no behavior beyond getters and setters. Add domain logic or consider using a value object.",
            )
            .arg("name", name.to_string())
            .arg("field_count", field_count.to_string());
            violations.push(self.new_violation(file, message, span));
        } else if total_methods > 0 {
            let accessor_ratio = accessor_count as f64 / total_methods as f64;
            if accessor_ratio >= self.settings.max_getter_setter_ratio && field_count > 0 {
                let message = Message::new(
                    "E0028:potential-data-class",
                    "\"{name}\" is a potential Data Class: {field_count} fields, {accessor_pct}% of methods are accessors (max: {max_pct}%). Consider encapsulating behavior.",
                )
                .arg("name", name.to_string())
                .arg("field_count", field_count.to_string())
                .arg("accessor_pct", format!("{:.0}", accessor_ratio * 100.0))
                .arg("max_pct", format!("{:.0}", self.settings.max_getter_setter_ratio * 100.0));
                violations.push(self.new_violation(file, message, span));
            }
        }
    }
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        true
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        }
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        match statement {
            Statement::Class(class) => {
                self.check_members(file, std::str::from_utf8(class.name.value).unwrap_or_default(), &class.members, class.span(), &mut violations);
            }
            Statement::Trait(t) => {
                self.check_members(file, std::str::from_utf8(t.name.value).unwrap_or_default(), &t.members, t.span(), &mut violations);
            }
            Statement::Enum(e) => {
                self.check_members(file, std::str::from_utf8(e.name.value).unwrap_or_default(), &e.members, e.span(), &mut violations);
            }
            _ => {}
        }

        violations
    }
}

fn is_accessor_method(method: &mago_syntax::cst::Method<'_>, name: &str) -> bool {
    let is_getter = name.starts_with("get") && name.len() > 3
        && name.chars().nth(3).is_some_and(|c| c.is_uppercase());
    let is_isser = (name.starts_with("is") || name.starts_with("has")) && name.len() > 2
        && name.chars().nth(2).is_some_and(|c| c.is_uppercase());
    let is_setter = name.starts_with("set") && name.len() > 3
        && name.chars().nth(3).is_some_and(|c| c.is_uppercase());

    if !is_getter && !is_isser && !is_setter {
        return false;
    }

    let param_count = method.parameter_list.parameters.len();

    if is_setter && param_count != 1 {
        return false;
    }
    if (is_getter || is_isser) && param_count > 0 {
        return false;
    }

    if let MethodBody::Concrete(block) = &method.body {
        let stmt_count = block.statements.iter().count();
        return stmt_count <= 2;
    }

    true
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn data_class() {
        let violations = analyze_file_for_rule("e28/data_class.php", CODE);
        assert!(violations.len().gt(&0));
    }

    #[test]
    fn real_class() {
        let violations = analyze_file_for_rule("e28/real_class.php", CODE);
        assert!(violations.len().eq(&0));
    }
}
