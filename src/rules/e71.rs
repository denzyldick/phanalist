use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::{call_arguments, function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0071";
static DESCRIPTION: &str = "Unsafe file upload";

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
                if !is_upload_function(&name) {
                    return;
                }

                let call = Call::Function(call.clone());
                let arguments = call_arguments(&call);
                if arguments.len() < 2 {
                    return;
                }

                // The destination path is the second argument.
                if let Some(destination) = arguments.get(1) {
                    if has_user_input(destination) {
                        let message = Message::new(
                            format!("E0071:unsafe-file-upload:{name}"),
                            "{}() moves an uploaded file to a user-controlled destination. Validate the target path to prevent path traversal.",
                        )
                        .arg("name", name);

                        violations.push(self.new_violation(file, message, call.span()));
                    }
                }
            });
        }

        Some(violations)
    }
}

fn is_upload_function(name: &str) -> bool {
    matches!(name, "move_uploaded_file" | "file_put_contents")
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

fn is_superglobal(name: &str) -> bool {
    matches!(name, "_GET" | "_POST" | "_REQUEST" | "_COOKIE" | "_FILES" | "_SERVER")
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_unsafe_file_upload() {
        let violations = analyze_file_for_rule("e71/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e71/safe.php", CODE).is_empty());
    }
}
