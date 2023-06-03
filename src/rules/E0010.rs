use crate::analyse::Rule;

pub struct E0010 {}

impl Rule for E0010 {
    fn validate(
        &self,
        _statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        vec![]
    }
}
