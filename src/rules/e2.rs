use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::Statement;

use crate::project::Suggestion;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0002")
    }

    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

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
                    suggestions.push(
                        Suggestion::from(
                            "There is an empty catch. It's not recommended to catch an Exception without doing anything with it..".to_string(),
                            *start,
                            self.get_code()
                        )
                    );
                }
            }
        };

        suggestions
    }
}
