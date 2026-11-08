use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::{call_arguments, walk_statement_expressions};

pub(crate) static CODE: &str = "E0079";
static DESCRIPTION: &str = "Dynamic method call";

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
                let Expression::Call(Call::Method(call)) = expression else {
                    return;
                };

                if !has_user_input(&call.object) {
                    return;
                }

                let method_call = Call::Method(call.clone());
                let arguments = call_arguments(&method_call);
                if arguments.iter().any(|arg| has_user_input(arg)) {
                    let message = Message::new(
                        "E0079:dynamic-method-call",
                        "Dynamic method call on a user-controlled object with user-controlled arguments can lead to arbitrary code execution.",
                    );
                    violations.push(self.new_violation(file, message, call.span()));
                }
            });
        }

        Some(violations)
    }
}

fn has_user_input(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::ArrayAccess(access) => {
            if let Expression::Variable(Variable::Direct(variable)) = access.array.unparenthesized() {
                true
            } else {
                false
            }
        }
        Expression::Variable(Variable::Direct(variable)) => {
            true
        }
        Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
            has_user_input(binary.lhs) || has_user_input(binary.rhs)
        }
        Expression::CompositeString(string) => {
            string
                .parts()
                .iter()
                .any(|part| matches!(part, StringPart::Expression(_) | StringPart::BracedExpression(_)))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_dynamic_method_call() {
        let violations = analyze_file_for_rule("e79/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e79/safe.php", CODE).is_empty());
    }
}
