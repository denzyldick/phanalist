use php_parser_rs::parser::ast::{Expression, MethodCallExpression, NewExpression, Statement};

use crate::file::File;
use crate::results::Violation;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0010")
    }

    fn description(&self) -> String {
        String::from("Example rule")
    }

    fn validate(&self, _file: &File, statement: &Statement) -> Vec<Violation> {
        if let Statement::Expression(expression_statement) = statement {
            let _class_name_target = match &expression_statement.expression {
                Expression::AssignmentOperation(assign) => match assign.right() {
                    Expression::New(NewExpression {
                        new: _,
                        target,
                        arguments: _,
                    }) => Some(target),
                    _ => None,
                },
                _ => None,
            };

            if let Expression::MethodCall(MethodCallExpression {
                target: _,
                arrow: _,
                method: _,
                arguments: _,
            }) = &expression_statement.expression
            {}
        };
        vec![]
    }
}
