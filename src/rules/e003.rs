use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::modifiers::MethodModifierGroup;
use php_parser_rs::parser::ast::Statement;

use crate::project::Suggestion;
use crate::rules::Rule;

pub struct E003 {}

impl Rule for E003 {
    fn get_code(&self) -> String {
        String::from("E003")
    }

    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                match member {
                    ClassMember::ConcreteMethod(concretemethod) => {
                        let method_name = &concretemethod.name.value;
                        let MethodModifierGroup { modifiers } = &concretemethod.modifiers;
                        if modifiers.is_empty() {
                            suggestions.push(Suggestion::from(
                                format!("The method {} has no modifiers.", method_name).to_string(),
                                concretemethod.function,
                                "E003".to_string(),
                            ))
                        };
                    }
                    ClassMember::ConcreteConstructor(constructor) => {
                        let method_name = &constructor.name.value;
                        let MethodModifierGroup { modifiers } = &constructor.modifiers;
                        if modifiers.is_empty() {
                            suggestions.push(Suggestion::from(
                                format!("This method {} has no modifiers.", method_name)
                                    .to_string(),
                                constructor.function,
                                "E003".to_string(),
                            ))
                        };
                    }
                    _ => {}
                }
            }
        };

        suggestions
    }
}
