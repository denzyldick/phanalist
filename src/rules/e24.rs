use mago_span::HasSpan;
use mago_syntax::ast::{ClassLikeMember, Statement, MethodBody};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0024";
static DESCRIPTION: &str = "Lines of Code (LOC) per Method";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_loc: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_loc: 30 }
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
                    if loc > self.settings.max_loc {
                        let suggestion = format!(
                            "Method \"{}\" has {} lines of code (max: {}). Consider breaking it into smaller methods.",
                            method.name.value, loc, self.settings.max_loc
                        );
                        violations.push(self.new_violation(file, suggestion, method.span()));
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
    fn long_method() {
        let violations = analyze_file_for_rule("e24/long_method.php", CODE);
        assert!(violations.len().gt(&0));
        assert!(violations[0].suggestion.contains("longMethod"));
    }

    #[test]
    fn short_method() {
        let violations = analyze_file_for_rule("e24/short_method.php", CODE);
        assert!(violations.len().eq(&0));
    }
}
