use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::modifiers::PropertyModifierGroup;
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0006";
static DESCRIPTION: &str = "Property modifiers";

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::Property(property) = member {
                    if Self::property_without_modifiers(property) {
                        let name = Self::property_name(property);
                        let suggestion =
                            format!("The variables {} have no modifier.", name.join(", "));
                        violations.push(self.new_violation(file, suggestion, property.end));
                    }
                }
            }
        };

        violations
    }
}

impl Rule {
    fn property_without_modifiers(property: &Property) -> bool {
        let PropertyModifierGroup { modifiers } = &property.modifiers;
        modifiers.is_empty()
    }
    fn property_name(property: &Property) -> Vec<std::string::String> {
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
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn no_method_modifiers() {
        let violations = analyze_file_for_rule("e6/no_var_modifiers.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "The variables $var have no modifier.".to_string()
        );
    }

    #[test]
    fn with_modifiers() {
        let violations = analyze_file_for_rule("e6/with_var_modifiers.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
