use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::{call_arguments, function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0098";
static DESCRIPTION: &str = "Session ID in URL";

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
                if name != "header" {
                    return;
                }

                let call = Call::Function(call.clone());
                let arguments = call_arguments(&call);
                if arguments.is_empty() {
                    return;
                }

                if arguments.iter().any(|arg| has_session_in_url(arg)) {
                    let message = Message::new(
                        "E0098:session-id-in-url",
                        "header() redirect includes a session ID in the URL, which can be leaked via Referer headers.",
                    );
                    violations.push(self.new_violation(file, message, call.span()));
                }
            });
        }

        Some(violations)
    }
}

fn has_session_in_url(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Variable(_) => true,
        Expression::CompositeString(string) => string.parts().iter().any(|part| {
            matches!(part, StringPart::Expression(_) | StringPart::BracedExpression(_))
        }),
        Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
            has_session_in_url(binary.lhs) || has_session_in_url(binary.rhs)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_session_id_in_url() {
        let violations = analyze_file_for_rule("e98/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e98/safe.php", CODE).is_empty());
    }
}
