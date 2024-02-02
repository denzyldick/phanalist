use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::constant::ConstantEntry;
use php_parser_rs::parser::ast::Statement;

use crate::analyse::Rule;
use crate::project::Suggestion;

pub struct E004 {}

impl Rule for E004 {
    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::Constant(constant) = member {
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
            }
        };

        suggestions
    }
}
/// Check if the constant name is entry all uppercase.
pub fn uppercased_constant_name(entry: ConstantEntry) -> bool {
    let ConstantEntry {
        name,
        equals: _,
        value: _,
    } = entry;

    let mut is_uppercase = true;
    for l in name.value.to_string().chars() {
        if !l.is_uppercase() && l.is_alphabetic() {
            is_uppercase = l.is_uppercase()
        }
    }

    is_uppercase
}
