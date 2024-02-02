use crate::analyse::Rule;
use crate::project::Suggestion;
use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::Statement;

pub struct E002 {}
impl Rule for E002 {
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
                            "E002".to_string()
                        )
                    );
                }
            }
        };

        suggestions
    }
}
