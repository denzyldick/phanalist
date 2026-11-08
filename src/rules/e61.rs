use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::{call_arguments, function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0061";
static DESCRIPTION: &str = "Cross-Site Scripting (XSS)";

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
            for nested in self.flatten_statements_to_validate(statement) {
                match nested {
                    Statement::Echo(echo) => {
                        for value in echo.values.iter() {
                            if has_user_input(value) {
                                let message = Message::new(
                                    "E0061:xss:echo",
                                    "echo() outputs user-controlled input without HTML escaping. Use htmlspecialchars() or a template engine with auto-escaping.",
                                );
                                violations.push(self.new_violation(file, message, value.span()));
                            }
                        }
                    }
                    Statement::EchoTag(echo_tag) => {
                        for value in echo_tag.values.iter() {
                            if has_user_input(value) {
                                let message = Message::new(
                                    "E0061:xss:short-echo-tag",
                                    "Short echo tag outputs user-controlled input without HTML escaping. Use htmlspecialchars() or a template engine with auto-escaping.",
                                );
                                violations.push(self.new_violation(file, message, value.span()));
                            }
                        }
                    }
                    _ => {}
                }
            }

            walk_statement_expressions(statement, &mut |expression| {
                match expression {
                    Expression::Construct(Construct::Print(print_)) => {
                        if has_user_input(&print_.value) {
                            let message = Message::new(
                                "E0061:xss:print",
                                "print() outputs user-controlled input without HTML escaping. Use htmlspecialchars() or a template engine with auto-escaping.",
                            );
                            violations.push(self.new_violation(file, message, print_.value.span()));
                        }
                    }
                    Expression::Call(Call::Function(call)) => {
                        let Some(name) = function_name(call.function) else {
                            return;
                        };
                        if !is_output_function(&name) {
                            return;
                        }
                        let call = Call::Function(call.clone());
                        let arguments = call_arguments(&call);
                        if arguments.iter().any(|arg| has_user_input(arg)) {
                            let message = Message::new(
                                format!("E0061:xss:{name}"),
                                "{}() outputs user-controlled input without HTML escaping. Use htmlspecialchars() or a template engine with auto-escaping.",
                            )
                            .arg("name", name);
                            violations.push(self.new_violation(file, message, call.span()));
                        }
                    }
                    _ => {}
                }
            });
        }

        Some(violations)
    }
}

fn is_output_function(name: &str) -> bool {
    matches!(name, "printf" | "vprintf")
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

fn variable_name(name: &[u8]) -> String {
    let name = String::from_utf8_lossy(name);
    name.strip_prefix('$').unwrap_or(&name).to_string()
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_xss() {
        let violations = analyze_file_for_rule("e61/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e61/safe.php", CODE).is_empty());
    }
}
