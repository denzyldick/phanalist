use crate::config::Config;
use crate::file::File;
use crate::results::Violation;
use mago_ast::ast::class_like::member::ClassLikeMember;
use mago_ast::{Modifier, Statement};
use mago_span::HasSpan;

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

    fn do_validate(&self, _file: &File) -> bool {
        true
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        match statement {
            Statement::Class(class) => {
                for member in class.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if !self.has_visibility_modifier(&method.modifiers) {
                            let method_name = file.interner.lookup(&method.name.value);
                            let suggestion = format!(
                                "Method name \"{}\" should be declared with a visibility modifier.",
                                method_name
                            );
                            violations.push(self.new_violation(file, suggestion, method.span()));
                        }
                    }
                }
            }
            Statement::Interface(interface) => {
                for member in interface.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if !self.has_visibility_modifier(&method.modifiers) {
                            let method_name = file.interner.lookup(&method.name.value);
                            let suggestion = format!(
                                "Method name \"{}\" should be declared with a visibility modifier.",
                                method_name
                            );
                            violations.push(self.new_violation(file, suggestion, method.span()));
                        }
                    }
                }
            }
            Statement::Trait(t) => {
                for member in t.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if !self.has_visibility_modifier(&method.modifiers) {
                            let method_name = file.interner.lookup(&method.name.value);
                            let suggestion = format!(
                                "Method name \"{}\" should be declared with a visibility modifier.",
                                method_name
                            );
                            violations.push(self.new_violation(file, suggestion, method.span()));
                        }
                    }
                }
            }
            _ => {}
        }

        violations
    }
}

impl Rule {
    fn has_visibility_modifier(&self, modifiers: &mago_ast::Sequence<Modifier>) -> bool {
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
