use mago_span::HasSpan;
use mago_syntax::cst::{ClassLikeMember, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0027";
static DESCRIPTION: &str = "God Class (Brain Class)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_methods: usize,
    pub max_fields: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_methods: 15,
            max_fields: 10,
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
        let mut method_count = 0;
        let mut field_count = 0;

        for member in members.iter() {
            match member {
                ClassLikeMember::Method(_) => method_count += 1,
                ClassLikeMember::Property(_) => field_count += 1,
                _ => {}
            }
        }

        if method_count > self.settings.max_methods && field_count > self.settings.max_fields {
            let message = Message::new(
                "E0027:god-class",
                "\"{name}\" is a potential God Class: {method_count} methods and {field_count} fields (max: {max_methods} methods, {max_fields} fields). Consider splitting it into multiple types with single responsibilities.",
            )
            .arg("name", name.to_string())
            .arg("method_count", method_count.to_string())
            .arg("field_count", field_count.to_string())
            .arg("max_methods", self.settings.max_methods.to_string())
            .arg("max_fields", self.settings.max_fields.to_string());
            violations.push(self.new_violation(file, message, span));
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

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn god_class() {
        let violations = analyze_file_for_rule("e27/god_class.php", CODE);
        assert!(violations.len().gt(&0));
        assert!(violations[0].message.render().contains("GodClass"));
    }

    #[test]
    fn cohesive_class() {
        let violations = analyze_file_for_rule("e27/cohesive_class.php", CODE);
        assert!(violations.len().eq(&0));
    }
}
