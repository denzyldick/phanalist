use mago_span::HasSpan;
use mago_syntax::ast::{ClassLikeMember, Modifier, Sequence, Statement};

use crate::file::File;
use crate::results::Violation;

pub struct Rule {}

const CODE: &str = "E0003";
const DESCRIPTION: &str = "Method modifiers";

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        true
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        let members = match statement {
            Statement::Class(class) => Some(&class.members),
            Statement::Interface(interface) => Some(&interface.members),
            Statement::Trait(t) => Some(&t.members),
            _ => None,
        };

        if let Some(members) = members {
            for member in members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    if !self.has_visibility_modifier(&method.modifiers) {
                        let suggestion = format!(
                            "Method name \"{}\" should be declared with a visibility modifier.",
                            method.name.value
                        );
                        violations.push(self.new_violation(file, suggestion, method.span()));
                    }
                }
            }
        }

        violations
    }
}

impl Rule {
    fn has_visibility_modifier(&self, modifiers: &Sequence<'_, Modifier<'_>>) -> bool {
        for modifier in modifiers.iter() {
            match modifier {
                Modifier::Public(_) | Modifier::Protected(_) | Modifier::Private(_) => return true,
                _ => {}
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn no_method_modifiers() {
        let violations = analyze_file_for_rule("e3/no_method_modifiers.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Method name \"methodWithoutModifier\" should be declared with a visibility modifier."
                .to_string()
        );
    }

    #[test]
    fn no_constructor_modifier() {
        let violations = analyze_file_for_rule("e3/no_constructor_modifiers.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Method name \"__construct\" should be declared with a visibility modifier."
                .to_string()
        );
    }

    #[test]
    fn with_modifiers() {
        let violations = analyze_file_for_rule("e3/with_modifiers.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
