use crate::analyse::Rule;
use crate::project::Suggestion;
use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::modifiers::MethodModifierGroup;
use php_parser_rs::parser::ast::Statement;

pub struct E003 {}

impl Rule for E003 {
    fn validate(&self, statement: &Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        match statement {
            Statement::Class(class) => {
                for member in &class.body.members {
                    match member {
                        ClassMember::ConcreteMethod(concretemethod) => {
                            let method_name = &concretemethod.name.value;
                            match &concretemethod.modifiers {
                                MethodModifierGroup { modifiers } => {
                                    if modifiers.len() == 0 {
                                        suggestions.push(Suggestion::from(
                                            format!("The method {} has no modifiers.", method_name)
                                                .to_string(),
                                            concretemethod.function,
                                        ))
                                    }
                                }
                            };
                        }
                        ClassMember::ConcreteConstructor(constructor) => {
                            let method_name = &constructor.name.value;
                            match &constructor.modifiers {
                                MethodModifierGroup { modifiers } => {
                                    if modifiers.len() == 0 {
                                        suggestions.push(Suggestion::from(
                                            format!(
                                                "This method {} has no modifiers.",
                                                method_name
                                            )
                                            .to_string(),
                                            constructor.function,
                                        ))
                                    }
                                }
                            };
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