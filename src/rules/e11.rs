use php_parser_rs::parser::ast::{
    ErrorSuppressExpression, Expression, ExpressionStatement, ReturnStatement, Statement,
};

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

    fn validate(&self, _file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violation = vec![];
        let flatten_statements = self.travers_statements_to_validate(vec![].clone(), statement);
        for statement in flatten_statements {
            match statement {
                Statement::Expression(ExpressionStatement {
                    expression: Expression::ErrorSuppress(ErrorSuppressExpression { at, expr: _ }),
                    ending: _,
                }) => {
                    let suggestion = "Error supression(@) symbol found. Remove it.".to_string();
                    violation.push(Violation {
                        rule: String::from(CODE),
                        line: at.line.to_string(),
                        suggestion,
                        span: *at,
                    });
                }
                Statement::Return(ReturnStatement {
                    r#return: _,
                    value: Some(Expression::ErrorSuppress(ErrorSuppressExpression { at, expr: _ })),
                    ending: _,
                }) => {
                    let suggestion = "Error supression(@) symbol found. Remove it. ".to_string();
                    violation.push(Violation {
                        rule: String::from(CODE),
                        line: at.line.to_string(),
                        suggestion,
                        span: *at,
                    });
                }
                _ => {}
            };
        }
        violation
    }
}
#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn example() {
        let violations = analyze_file_for_rule("e11/detect_@.php", CODE);

        assert!(violations.len().gt(&1));
    }
}
