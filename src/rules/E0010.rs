use std::collections::HashMap;

use php_parser_rs::parser::ast::{
    identifiers::SimpleIdentifier, variables::SimpleVariable, ExpressionStatement,
    MethodCallExpression, NewExpression, Statement,
};

use crate::{analyse::Rule, project::File};
///
pub struct E0010 {
    pub file: crate::project::File,
    scope: HashMap<String, SimpleIdentifier>,
}
impl E0010 {
    pub fn new(file: File) -> E0010 {
        E0010 {
            file: file,
            scope: HashMap::new(),
        }
    }
}
impl Rule for E0010 {
    fn validate(
        &self,
        statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        match statement {
            php_parser_rs::parser::ast::Statement::Expression(ExpressionStatement) => {
                let class_name_target = match &ExpressionStatement.expression {
                    php_parser_rs::parser::ast::Expression::AssignmentOperation(Assign) => {
                        match Assign.right() {
                            php_parser_rs::parser::ast::Expression::New(NewExpression {
                                new,
                                target,
                                arguments,
                            }) => Some(target),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                println!("{:#?}", self.file.members.len());
                match &ExpressionStatement.expression {
                    php_parser_rs::parser::ast::Expression::MethodCall(MethodCallExpression {
                        target,
                        arrow,
                        method,
                        arguments,
                    }) => {}
                    _ => {}
                }
            }
            _ => {}
        };
        vec![]
    }
}
