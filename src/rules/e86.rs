use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::{function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0086";
static DESCRIPTION: &str = "Deprecated session function";

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
                if !is_deprecated_session(&name) {
                    return;
                }

                let call = Call::Function(call.clone());
                let message = Message::new(
                    format!("E0086:deprecated-session:{name}"),
                    "{}() is a deprecated session function. Use the $_SESSION superarray instead.",
                )
                .arg("name", name);

                violations.push(self.new_violation(file, message, call.span()));
            });
        }

        Some(violations)
    }
}

fn is_deprecated_session(name: &str) -> bool {
    matches!(
        name,
        "session_register" | "session_is_registered" | "session_unregister"
    )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_deprecated_session() {
        let violations = analyze_file_for_rule("e86/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e86/safe.php", CODE).is_empty());
    }
}
