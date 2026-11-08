use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::walk_statement_expressions;

pub(crate) static CODE: &str = "E0087";
static DESCRIPTION: &str = "Command execution with backticks";

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
                match expression {
                    Expression::CompositeString(CompositeString::ShellExecute(shell)) => {
                        if shell.parts.iter().any(|part| {
                            matches!(part, StringPart::Expression(_) | StringPart::BracedExpression(_))
                        }) {
                            let message = Message::new(
                                "E0087:backtick-command",
                                "Backtick command execution with interpolated input can lead to command injection. Use escapeshellarg() on user input.",
                            );
                            violations.push(self.new_violation(file, message, expression.span()));
                        }
                    }
                    _ => {}
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
    fn reports_backtick_command() {
        let violations = analyze_file_for_rule("e87/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e87/safe.php", CODE).is_empty());
    }
}
