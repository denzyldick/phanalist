use php_parser_rs::parser::ast::{
    classes::ClassMember,
    functions::{ConstructorParameterList, FunctionParameterList},
    Statement,
};
use serde_json::Value;

use crate::project::Suggestion;
use crate::rules::Rule as Base;

pub struct Settings {
    pub max_parameters: i32,
}
pub struct Rule {
    pub settings: Settings,
}

impl Default for Rule {
    fn default() -> Self {
        Rule {
            settings: Settings { max_parameters: 5 },
        }
    }
}

impl Base for Rule {
    fn get_code(&self) -> String {
        String::from("E007")
    }

    fn set_config(&mut self, _json: &Value) {
        dbg!(_json);
    }

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
