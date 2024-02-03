use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::modifiers::PropertyModifierGroup;
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::Statement;

use crate::project::Suggestion;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0006")
    }

    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::Property(property) = member {
                    let name = property_name(property.clone());
                    if property_without_modifiers(property.clone()) {
                        suggestions.push(Suggestion::from(
                            format!("The variables {} have no modifier.", name.join(", "))
                                .to_string(),
                            property.end,
                            self.get_code(),
                        ));
                    }
                }
            }
        };

        suggestions
    }
}
/// Check if the porperty has a modifier.
pub fn property_without_modifiers(property: Property) -> bool {
    let PropertyModifierGroup { modifiers } = property.modifiers;
    modifiers.is_empty()
}
fn property_name(property: Property) -> Vec<std::string::String> {
    let Property {
        attributes: _,
        modifiers: _,
        r#type: _,
        entries,
        end: _,
    } = property;
    {
        let mut names: Vec<String> = Vec::new();
        for entry in entries {
            let name = match entry {
                PropertyEntry::Initialized {
                    variable,
                    equals: _,
                    value: _,
                } => variable.name.to_string(),
                PropertyEntry::Uninitialized { variable } => variable.to_string(),
            };
            names.push(name);
        }
        names
    }
}
