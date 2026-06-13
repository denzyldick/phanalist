use mago_span::HasSpan;
use mago_syntax::ast::{ClassLikeMember, Statement, MethodBody};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::e9::calculate_complexity;
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0030";
static DESCRIPTION: &str = "Cyclomatic Complexity Density";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_density: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_density: 0.3 }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl Rule {
    fn check_methods(
        &self,
        file: &File<'_>,
        members: &mago_syntax::ast::Sequence<'_, ClassLikeMember<'_>>,
        violations: &mut Vec<Violation>,
    ) {
        for member in members.iter() {
            if let ClassLikeMember::Method(method) = member {
                if let MethodBody::Concrete(block) = &method.body {
                    let start_line = file.line_number(block.span().start.offset);
                    let end_line = file.line_number(block.span().end.offset);

                    if end_line <= start_line + 1 {
                        continue;
                    }
                    let loc = end_line - start_line - 1;

                    let complexity = 1 + calculate_complexity(&block.statements);
                    let density = complexity as f64 / loc as f64;

                    if density > self.settings.max_density {
                        let message = Message::new(
                            "E0030:complexity-density",
                            "Method \"{name}\" has complexity density of {density} (max: {max_density}). Complexity {complexity} in {loc} lines. Consider simplifying the logic.",
                        )
                        .arg("name", String::from_utf8_lossy(method.name.value).to_string())
                        .arg("density", format!("{:.2}", density))
                        .arg("max_density", format!("{:.2}", self.settings.max_density))
                        .arg("complexity", complexity.to_string())
                        .arg("loc", loc.to_string());
                        violations.push(self.new_violation(file, message, method.span()));
                    }
                }
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
                self.check_methods(file, &class.members, &mut violations);
            }
            Statement::Trait(t) => {
                self.check_methods(file, &t.members, &mut violations);
            }
            Statement::Enum(e) => {
                self.check_methods(file, &e.members, &mut violations);
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
    fn dense_method() {
        let violations = analyze_file_for_rule("e30/dense_method.php", CODE);
        assert!(violations.len().gt(&0));
        assert!(violations[0].message.render().contains("denseMethod"));
    }

    #[test]
    fn sparse_method() {
        let violations = analyze_file_for_rule("e30/sparse_method.php", CODE);
        assert!(violations.len().eq(&0));
    }
}
