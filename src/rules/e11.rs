use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0011")
    }

    fn description(&self) -> String {
        String::from("Example rule")
    }

    fn validate(&self, _file: &File, _statement: &Statement) -> Vec<Violation> {
        vec![]
    }
}
