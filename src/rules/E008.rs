use php_parser_rs::parser::ast::{
    classes::ClassMember, functions::MethodBody, ReturnStatement, Statement,
};

use crate::{analyse::Rule, project::Suggestion};

pub struct E008 {}

impl Rule for E008 {
    fn validate(
        &self,
        statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        let mut suggestions = Vec::new();
        match statement {
            Statement::Class(class) => {
                for member in &class.body.members {
                    match member {
                        ClassMember::ConcreteMethod(concretemethod) => {
                            // Detect return statement without the proper return type signature.
                            let has_return = method_has_return(concretemethod.body.clone());
                            let method_name = &concretemethod.name.value;

                            match has_return {
                                Some(ReturnStatement {
                                    r#return,
                                    value: _,
                                    ending: _,
                                }) => {
                                    match concretemethod.return_type {
                                        None => {
                                            suggestions.push(
                                                                Suggestion::from(
                                                                    format!("The {} has a return statement but it has no return type signature.", method_name).to_string(),
                                                                r#return,
                                                        "E008".to_string(),

                                                                )
                                                            );
                                        }
                                        _ => {}
                                    };
                                }
                                None => {}
                            };
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        suggestions
    }
}
/// Return the type of method body.  
fn method_has_return(body: MethodBody) -> Option<ReturnStatement> {
    let mut r: Option<ReturnStatement> = None;
    for statement in body.statements {
        match statement {
            Statement::Return(ReturnStatement {
                r#return,
                value,
                ending,
            }) => {
                r = Some(ReturnStatement {
                    r#return,
                    value,
                    ending,
                });
            }
            _ => {}
        };
    }
    r
}
