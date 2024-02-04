use php_parser_rs::parser::ast::{
    classes::ClassMember,
    functions::{ConstructorParameterList, FunctionParameterList},
    Statement,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;

pub static CODE: &str = "E0007";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_parameters: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_parameters: 5 }
    }
}

pub struct Rule {
    pub settings: Settings,
}

impl Default for Rule {
    fn default() -> Self {
        Rule {
            settings: Settings {
                ..Default::default()
            },
        }
    }
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn set_config(&mut self, json: &Value) {
        if let Ok(settings) = serde_json::from_value(json.to_owned()) {
            self.settings = settings;
        }
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        let suggestion  = format!("This method has too many parameters. More than {} parameters is considered a too much.", self.settings.max_parameters);
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
                        if parameters.inner.len() > self.settings.max_parameters as usize {
                            violations.push(self.new_violation(
                                file,
                                suggestion.clone(),
                                concretemethod.function,
                            ));
                        }
                    }
                    ClassMember::ConcreteConstructor(concreteconstructor) => {
                        let ConstructorParameterList {
                            comments: _,
                            left_parenthesis: _,
                            right_parenthesis: _,
                            parameters,
                        } = &concreteconstructor.parameters;
                        if parameters.inner.len() > self.settings.max_parameters as usize {
                            violations.push(self.new_violation(
                                file,
                                suggestion.clone(),
                                concreteconstructor.function,
                            ));
                        }
                    }
                    _ => {}
                }
            }
        };
        violations
    }
}
