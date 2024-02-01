use crate::analyse::Rule;
use crate::project::Suggestion;
use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::constant::ConstantEntry;
use php_parser_rs::parser::ast::Statement;

pub struct E004 {}

impl Rule for E004 {
    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        match statement {
            Statement::Class(class) => {
                for member in &class.body.members {
                    match member {
                        ClassMember::Constant(constant) => {
                            for entry in &constant.entries {
                                if !uppercased_constant_name(entry.clone()) {
                                    suggestions.push(Suggestion::from(
                                        format!(
                                            "All letters in a constant({}) should be uppercased.",
                                            entry.name.value
                                        ),
                                        entry.name.span,
                                        "E004".to_string(),
                                    ))
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        suggestions
    }
}
/// Check if the constant name is entry all uppercase.
pub fn uppercased_constant_name(entry: ConstantEntry) -> bool {
    match entry {
        ConstantEntry {
            name,
            equals: _,
            value: _,
        } => {
            let mut is_uppercase = true;
            for l in name.value.to_string().chars() {
                if !l.is_uppercase() && l.is_alphabetic() {
                    is_uppercase = l.is_uppercase()
                }
            }

            is_uppercase
        }
    }
}
