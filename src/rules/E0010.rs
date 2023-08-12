use php_parser_rs::parser::ast::{
    ExpressionStatement, MethodCallExpression, NewExpression, Statement,
};

use crate::analyse::Rule;

pub struct E0010 {
    pub file: crate::project::File,
}

impl Rule for E0010 {
    fn validate(
        &self,
        statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        match statement {
            php_parser_rs::parser::ast::Statement::Expression(ExpressionStatement) => {
                // let class_name_target = match &ExpressionStatement.expression {
                //     php_parser_rs::parser::ast::Expression::AssignmentOperation(Assign) => {
                //         match Assign.right() {
                //             php_parser_rs::parser::ast::Expression::New(NewExpression {
                //                 new,
                //                 target,
                //                 arguments,
                //             }) => Some(target),
                //             _ => None,
                //         }
                //     }
                //     _ => None,
                // };
                // println!("{:#?}", self.file);
                // let class = ExpressionStatement.expression
                match &ExpressionStatement.expression {
                    php_parser_rs::parser::ast::Expression::MethodCall(MethodCallExpression {
                        target,
                        arrow,
                        method,
                        arguments,
                    }) => {
                        // println!("{:#?}", class_name_target);
                        // prin;
                        // println!("{:#?}", self.file.ast);
                        /**
                        @todo get the fully qualified name of the target class.
                        Retrieve the metadata from the datastore.
                        */

                        /**
                        The file struct is now available in the analyse struct.
                        Find a way that the rule can say if they need the whole file.
                        Like and extra interface or trait.
                        */
                        println!("{:#? }", target);
                        println!("{:#?}", method);
                    }
                    _ => {}
                }
            }
            _ => {}
        };
        vec![]
    }
}
