use crate::rules::File;
use crate::rules::Suggestion;

use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::classes::{ClassExtends, ClassMember, ClassStatement};
use php_parser_rs::parser::ast::constant::ConstantEntry;
use php_parser_rs::parser::ast::constant::ConstantStatement;
use php_parser_rs::parser::ast::control_flow::IfStatement;
use php_parser_rs::parser::ast::functions::{
    FunctionParameter, FunctionParameterList, MethodBody, ReturnType,
};
use php_parser_rs::parser::ast::identifiers::{DynamicIdentifier, Identifier, SimpleIdentifier};
use php_parser_rs::parser::ast::literals::Literal;
use php_parser_rs::parser::ast::modifiers::{
    MethodModifier, MethodModifierGroup, PropertyModifier, PropertyModifierGroup,
};
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::Block;
use php_parser_rs::parser::ast::BlockStatement;
use php_parser_rs::parser::ast::Expression;
use php_parser_rs::parser::ast::Statement;
use php_parser_rs::parser::ast::{operators, ReturnStatement};
/// All class names should be capatilized.
pub fn has_capitalized_name(name: String, span: Span) -> Option<Suggestion> {
    if name.chars().next().unwrap().is_uppercase() == false {
        Some(Suggestion::from(
                format!("The class name {} is not capitlized. The first letter of the name of the class should be in uppercase.", name).to_string(),
                span
            ))
    } else {
        None
    }
}

/// Check if a property exists.
pub fn propperty_exists(
    identifier: php_parser_rs::parser::ast::identifiers::Identifier,
    file: File,
) -> bool {
    match identifier {
        php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(identifier) => {
            match (identifier) {
                SimpleIdentifier { span, value } => {
                    let property_value = value;
                    for m in &file.members {
                        match m {
                            ClassMember::Property(p) => {
                                for entry in &p.entries {
                                    match (entry) {
                                        PropertyEntry::Uninitialized { variable } => {
                                            return variable.name.to_string()
                                                == format!("${}", property_value);
                                        }
                                        PropertyEntry::Initialized {
                                            variable,
                                            equals,
                                            value,
                                        } => {
                                            return variable.name.to_string()
                                                == format!("${}", property_value);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        php_parser_rs::parser::ast::identifiers::Identifier::DynamicIdentifier(_) => {}
    }
    false
}
/// Retrieve the property name.
pub fn get_property_name(
    identifier: php_parser_rs::parser::ast::identifiers::Identifier,
) -> String {
    match identifier.clone() {
        php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(identifier) => {
            identifier.value.to_string()
        }
        _ => "".to_string(),
    }
}

/// Check if the porperty has a modifier.
pub fn property_without_modifiers(property: Property) -> bool {
    return match property.modifiers {
        PropertyModifierGroup { modifiers } => return modifiers.len() == 0,
    };
}

/// Check if the constant name is entry all uppercase.
pub fn uppercased_constant_name(entry: ConstantEntry) -> bool {
    match entry {
        ConstantEntry {
            name,
            equals,
            value,
        } => {
            let mut is_uppercase = true;
            for l in name.value.to_string().chars() {
                if l.is_uppercase() == false && l.is_alphabetic() {
                    is_uppercase = l.is_uppercase()
                }
            }

            return is_uppercase;
        }
    }
}

/// Check if the parameter without defining a type.
pub fn function_parameter_without_type(parameter: FunctionParameter) -> bool {
    match parameter {
        FunctionParameter {
            comments,
            name,
            attributes,
            data_type,
            ellipsis,
            default,
            ampersand,
        } => match data_type {
            None => return true,
            Some(_) => return false,
        },
    }
}

/// Return the type of method body.  
pub fn method_has_return(body: MethodBody) -> Option<ReturnStatement> {
    for statement in body.statements {
        return match statement {
            Statement::Return(ReturnStatement {
                r#return,
                value,
                ending,
            }) => match value {
                Some(ref s) => match s {
                    Expression::Literal(l) => {
                        return Some(ReturnStatement {
                            r#return: r#return,
                            value: value,
                            ending: ending,
                        })
                    }
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        };
    }
    None
}
