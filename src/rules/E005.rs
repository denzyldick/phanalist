use crate::analyse::Rule;
use crate::project::Suggestion;
use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::Statement;

pub struct E005 {}
impl Rule for E005 {
    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        match statement {
            Statement::Class(class) => {
                let name = String::from(class.name.value.clone());
                match has_capitalized_name(name.clone(), class.class) {
                    Some(s) => {
                        suggestions.push(s);
                    }
                    None => {}
                }
            }
            _ => {}
        }
        suggestions
    }
}
pub fn has_capitalized_name(name: String, span: Span) -> Option<Suggestion> {
    if name.chars().next().unwrap().is_uppercase() == false {
        Some(Suggestion::from(
                format!("The class name {} is not capitlized. The first letter of the name of the class should be in uppercase.", name).to_string(),
                span,
                "E005".to_string()
            ))
    } else {
        None
    }
}
