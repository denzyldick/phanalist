use crate::analyse::Rule;
use crate::project::Suggestion;
use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::modifiers::PropertyModifierGroup;
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::Statement;

pub struct E006 {}
impl Rule for E006 {
    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        match statement {
            Statement::Class(class) => {
                for member in &class.body.members {
                    match member {
                        ClassMember::Property(property) => {
                            let name = property_name(property.clone());
                            if property_without_modifiers(property.clone()) {
                                suggestions.push(Suggestion::from(
                                    format!("The variables {} have no modifier.", name.join(", "))
                                        .to_string(),
                                    property.end,
                                    "E006".to_string(),
                                ));
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
/// Check if the porperty has a modifier.
pub fn property_without_modifiers(property: Property) -> bool {
    match property.modifiers {
        PropertyModifierGroup { modifiers } => modifiers.is_empty(),
    }
}
fn property_name(property: Property) -> Vec<std::string::String> {
    match property {
        Property {
            attributes: _,
            modifiers: _,
            r#type: _,
            entries,
            end: _,
        } => {
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
}
