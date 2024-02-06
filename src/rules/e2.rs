use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0002")
    }

    fn description(&self) -> String {
        String::from("Empty catch")
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Try(s) = statement {
            for catch in &s.catches {
                let CatchBlock {
                    start,
                    end: _,
                    types: _,
                    var: _,
                    body,
                } = catch;
                if body.is_empty() {
                    let suggestion= String::from("There is an empty catch. It's not recommended to catch an Exception without doing anything with it..");
                    violations.push(self.new_violation(file, suggestion, *start));
                }
            }
        };

        violations
    }
}
