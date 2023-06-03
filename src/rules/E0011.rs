use crate::analyse::Rule;

pub struct E0011 {}

impl Rule for E0011 {
    fn validate(
        &self,
        statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        vec![]
    }
}
