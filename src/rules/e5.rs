use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

pub static CODE: &str = "E0005";

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from("Class name")
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            let name = String::from(class.name.value.clone());
            if !name.chars().next().unwrap().is_uppercase() {
                let suggestion = format!("The class name {} is not capitalized. The first letter of the name of the class should be in uppercase.", name);
                violations.push(self.new_violation(file, suggestion, class.class))
            }
        };

        violations
    }
}
