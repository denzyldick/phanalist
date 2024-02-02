use crate::analyse::Rule;
use crate::project::Suggestion;
use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::Statement;

pub struct E005 {}
impl Rule for E005 {
    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        if let Statement::Class(class) = statement {
            let name = String::from(class.name.value.clone());
            if let Some(s) = has_capitalized_name(name.clone(), class.class) {
                suggestions.push(s);
            }
        };

        suggestions
    }
}
pub fn has_capitalized_name(name: String, span: Span) -> Option<Suggestion> {
    if !name.chars().next().unwrap().is_uppercase() {
        Some(
            Suggestion::from(
                format!("The class name {} is not capitlized. The first letter of the name of the class should be in uppercase.", name).to_string(),
                span,
                "E005".to_string()
            )
        )
    } else {
        None
    }
}
