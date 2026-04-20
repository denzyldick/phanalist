use mago_span::HasSpan;
use mago_syntax::ast::{Call, Expression, Statement, UnaryPrefixOperator};

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0011";
static DESCRIPTION: &str = "Detect the error suppression symbol: @";

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
        true
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();
        let flatten_statements = self.flatten_statements_to_validate(statement);

        for stmt in flatten_statements {
            if let Statement::Expression(expr_stmt) = stmt {
                check_expression(file, self, expr_stmt.expression, &mut violations);
            }
            if let Statement::Return(ret) = stmt {
                if let Some(value) = ret.value {
                    check_expression(file, self, value, &mut violations);
                }
            }
        }

        violations
    }
}

fn check_expression(
    file: &File<'_>,
    rule: &dyn crate::rules::Rule,
    expr: &Expression<'_>,
    violations: &mut Vec<Violation>,
) {
    match expr {
        Expression::UnaryPrefix(prefix) => {
            if let UnaryPrefixOperator::ErrorControl(_) = prefix.operator {
                let suggestion = "Error supression(@) symbol found. Remove it.".to_string();
                violations.push(rule.new_violation(file, suggestion, prefix.span()));
            }
            check_expression(file, rule, prefix.operand, violations);
        }
        Expression::Call(call) => {
            if let Call::Method(m) = call {
                check_expression(file, rule, m.object, violations);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e11/detect_@.php", CODE);

        assert!(violations.len().gt(&0));
    }
}
