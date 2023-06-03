use crate::analyse::Rule;
use crate::project::Suggestion;
use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::Statement;

pub struct E002 {}
impl Rule for E002 {
    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        match statement {
            Statement::Try(s) => {
                for catch in &s.catches {
                    match catch {
                        CatchBlock {
                            start,
                            end: _,
                            types: _,
                            var: _,
                            body,
                        } => {
                            if body.len() == 0 {
                                suggestions.push(Suggestion::from("There is an empty catch. It's not recommended to catch an Exception without doing anything with it..".to_string(),*start ));
                            }
                        }
                    }
                }
            }
            _ => {}
        };

        suggestions
    }
}
