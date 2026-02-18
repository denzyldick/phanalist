use mago_ast::ast::class_like::member::ClassLikeMember;
use mago_ast::ast::class_like::property::Property;
use mago_ast::ast::modifier::Modifier;
use mago_ast::ast::Statement;
use mago_span::HasSpan;

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

    fn do_validate(&self, _file: &File) -> bool {
        true
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            for member in class.members.iter() {
                if let ClassLikeMember::Property(property) = member {
                    if Self::property_without_visibility(property) {
                        let names: Vec<String> = property
                            .variables()
                            .iter()
                            .map(|v| file.interner.lookup(&v.name).to_string())
                            .collect();

                        let suggestion =
                            format!("The variables {} have no modifier.", names.join(", "));
                        violations.push(self.new_violation(file, suggestion, property.span()));
                    }
                }
            }
        }

        violations
    }
}

impl Rule {
    fn property_without_visibility(property: &Property) -> bool {
        !property.modifiers().iter().any(|m| {
            matches!(
                m,
                Modifier::Public(_) | Modifier::Protected(_) | Modifier::Private(_)
            )
        })
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
