use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::modifiers::MethodModifierGroup;
use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0003";
static DESCRIPTION: &str = "Method modifiers";

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
                match member {
                    ClassMember::ConcreteMethod(method) => {
                        let MethodModifierGroup { modifiers } = &method.modifiers;
                        if modifiers.is_empty() {
                            let suggestion =
                                format!("The method {} has no modifiers.", &method.name.value);
                            violations.push(self.new_violation(file, suggestion, method.function))
                        };
                    }
                    ClassMember::ConcreteConstructor(constructor) => {
                        let MethodModifierGroup { modifiers } = &constructor.modifiers;
                        if modifiers.is_empty() {
                            let suggestion = format!(
                                "This method {} has no modifiers.",
                                &constructor.name.value
                            );
                            violations.push(self.new_violation(
                                file,
                                suggestion,
                                constructor.function,
                            ))
                        };
                    }
                    _ => {}
                }
            }
        };

        violations
    }
    fn travers_statements_to_validate<'a>(
        &'a self,
        flatten_statements: Vec<&'a Statement>,
        statement: &'a Statement,
    ) -> Vec<&Statement> {
        self.class_statements_only_to_validate(flatten_statements, statement)
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
            "The method methodWithoutModifier has no modifiers.".to_string()
        );
    }

    #[test]
    fn no_constructor_modifier() {
        let violations = analyze_file_for_rule("e3/no_constructor_modifiers.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "This method __construct has no modifiers.".to_string()
        );
    }

    #[test]
    fn with_modifiers() {
        let violations = analyze_file_for_rule("e3/with_modifiers.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
