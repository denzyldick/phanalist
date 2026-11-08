use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::{call_arguments, function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0060";
static DESCRIPTION: &str = "SQL injection";

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
                if !is_sql_function(&name) {
                    return;
                }

                let call = Call::Function(call.clone());
                let arguments = call_arguments(&call);
                if arguments.is_empty() {
                    return;
                }

                if arguments.iter().any(|arg| has_user_input(arg)) {
                    let message = Message::new(
                        format!("E0060:sql-injection:{name}"),
                        "{}() executes a SQL query built from user-controlled input. Use prepared statements with bound parameters instead.",
                    )
                    .arg("name", name);

                    violations.push(self.new_violation(file, message, call.span()));
                }
            });
        }

        Some(violations)
    }
}

fn is_sql_function(name: &str) -> bool {
    matches!(
        name,
        "mysqli_query" | "mysqli::query" | "pg_query" | "pg_exec" | "pdo::query" | "pdo::exec" | "mysql_query"
    )
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
    fn reports_sql_injection() {
        let violations = analyze_file_for_rule("e60/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e60/safe.php", CODE).is_empty());
    }
}
