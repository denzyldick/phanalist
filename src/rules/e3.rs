use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::modifiers::MethodModifierGroup;
use php_parser_rs::parser::ast::Statement;

use crate::file::File;
use crate::results::Violation;

pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from("E0003")
    }

    fn description(&self) -> String {
        String::from("Method modifiers")
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                match member {
                    ClassMember::ConcreteMethod(concretemethod) => {
                        let method_name = &concretemethod.name.value;
                        let MethodModifierGroup { modifiers } = &concretemethod.modifiers;
                        if modifiers.is_empty() {
                            let suggestion =
                                format!("The method {} has no modifiers.", method_name);
                            violations.push(self.new_violation(
                                file,
                                suggestion,
                                concretemethod.function,
                            ))
                        };
                    }
                    ClassMember::ConcreteConstructor(constructor) => {
                        let method_name = &constructor.name.value;
                        let MethodModifierGroup { modifiers } = &constructor.modifiers;
                        if modifiers.is_empty() {
                            let suggestion =
                                format!("This method {} has no modifiers.", method_name);
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
}
