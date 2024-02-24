use php_parser_rs::parser::ast::{
    classes::ClassMember,
    functions::{ConstructorParameterList, FunctionParameterList},
    Statement,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;

pub(crate) static CODE: &str = "E0007";
static DESCRIPTION: &str = "Method parameters count";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub check_constructor: bool,
    pub max_parameters: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            check_constructor: false,
            max_parameters: 8,
        }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn set_config(&mut self, json: &Value) {
        if let Ok(settings) = serde_json::from_value(json.to_owned()) {
            self.settings = settings;
        }
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                match member {
                    ClassMember::ConcreteMethod(method) => {
                        // Detect parameters without type.
                        let FunctionParameterList {
                            comments: _,
                            left_parenthesis: _,
                            right_parenthesis: _,
                            parameters,
                        } = &method.parameters;
                        if parameters.inner.len() > self.settings.max_parameters as usize {
                            let suggestion = format!("Method {} has too many parameters. More than {} parameters is considered a too much.", &method.name.value, self.settings.max_parameters);
                            violations.push(self.new_violation(file, suggestion, method.function));
                        }
                    }
                    ClassMember::ConcreteConstructor(constructor) => {
                        let ConstructorParameterList {
                            comments: _,
                            left_parenthesis: _,
                            right_parenthesis: _,
                            parameters,
                        } = &constructor.parameters;
                        if self.settings.check_constructor
                            && parameters.inner.len() > self.settings.max_parameters as usize
                        {
                            let suggestion  = format!("Constructor has too many parameters. More than {} parameters is considered a too much.", self.settings.max_parameters);
                            violations.push(self.new_violation(
                                file,
                                suggestion,
                                constructor.function,
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

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn method_max_params() {
        let violations = analyze_file_for_rule("e7/method_max_params.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Method test has too many parameters. More than 8 parameters is considered a too much."
                .to_string()
        );
    }

    #[test]
    fn constructor_max_params() {
        let violations = analyze_file_for_rule("e7/constructor_max_params.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn valid_amount_of_params() {
        let violations = analyze_file_for_rule("e7/valid_amount_of_params.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
