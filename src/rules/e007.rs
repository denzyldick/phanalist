use php_parser_rs::parser::ast::{
    classes::ClassMember,
    functions::{ConstructorParameterList, FunctionParameterList},
    Statement,
};

use crate::{analyse::Rule, project::Suggestion};

pub struct E007 {}
impl Rule for E007 {
    fn validate(
        &self,
        statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        let mut suggestions = Vec::new();

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                match member {
                    ClassMember::ConcreteMethod(concretemethod) => {
                        // Detect parameters without type.
                        let FunctionParameterList {
                            comments: _,
                            left_parenthesis: _,
                            right_parenthesis: _,
                            parameters,
                        } = &concretemethod.parameters;
                        if parameters.inner.len() > 5 {
                            suggestions.push(
                                Suggestion::from(
                                    "This method has too many parameters. More than 5 parameters is considered a too much. Try passing an object containing these values.".to_string(),
                                    concretemethod.function,
                                    "E007".to_string()
                                )
                            );
                        }
                    }
                    ClassMember::ConcreteConstructor(concreteconstructor) => {
                        let ConstructorParameterList {
                            comments: _,
                            left_parenthesis: _,
                            right_parenthesis: _,
                            parameters,
                        } = &concreteconstructor.parameters;
                        if parameters.inner.len() > 5 {
                            suggestions.push(
                                Suggestion::from(
                                    "This method has too many parameters. More than 5 parameters is considered a too much. Try passing an object containing these values.".to_string(),
                                    concreteconstructor.function,
                                    "E007".to_string()
                                )
                            );
                        }
                    }
                    _ => {}
                }
            }
        };
        suggestions
    }
}
