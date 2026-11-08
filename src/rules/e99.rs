use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::{call_arguments, function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0099";
static DESCRIPTION: &str = "Disable functions bypass";

#[derive(Default)]
pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        false
    }

    fn validate(&self, _file: &File<'_>, _statement: &Statement<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn validate_file(&self, file: &File<'_>) -> Option<Vec<Violation>> {
        let program = file.ast?;
        let mut violations = Vec::new();

        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| {
                let Expression::Call(Call::Function(call)) = expression else {
                    return;
                };
                let Some(name) = function_name(call.function) else {
                    return;
                };
                if name != "ini_set" {
                    return;
                }

                let call = Call::Function(call.clone());
                let arguments = call_arguments(&call);
                if arguments.is_empty() {
                    return;
                }
                let Some(first) = arguments.first() else { return };
                if let Some(value) = crate::rules::ast_support::literal_string(first) {
                    if value == "disable_functions" || value == "disable_classes" {
                        let message = Message::new(
                            "E0099:disable-functions-bypass",
                            "ini_set() attempts to change disable_functions or disable_classes, which can bypass security restrictions.",
                        );
                        violations.push(self.new_violation(file, message, call.span()));
                    }
                }
            });
        }

        Some(violations)
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_disable_functions_bypass() {
        let violations = analyze_file_for_rule("e99/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e99/safe.php", CODE).is_empty());
    }
}
