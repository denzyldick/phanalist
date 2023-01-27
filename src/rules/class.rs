
pub fn has_capitalized_name(name: String) {
    if !name.chars().next().unwrap().is_uppercase() {
        println!("The class name {} is not capitlized. The first letter of the name of the class should be in uppercase.", name);
    }
}

pub mod members {
    use php_parser_rs::parser::ast::classes::ClassMember;

    use crate::rules;

    pub fn analyze(member: ClassMember) {
        match member {
            ClassMember::Property(property) => {
                if property.modifiers.modifiers.len() == 0 {}
                for modifier in property.modifiers.modifiers {}
            }
            ClassMember::Constant(_constant) => {}
            ClassMember::TraitUsage(_trait) => {}
            ClassMember::AbstractMethod(abstractmethod) => {}
            ClassMember::ConcreteMethod(concretemethod) => {}
            ClassMember::VariableProperty(variableproperty) => {}
            ClassMember::AbstractConstructor(_constructor) => {}
            ClassMember::ConcreteConstructor(constructor) => {
                for statement in constructor.body.statements {
                    rules::analyze(statement);
                }
            }
        }
    }
}
