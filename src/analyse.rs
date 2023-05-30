use std::collections::HashMap;

use crate::project::File;
use crate::project::Suggestion;

use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser;
use php_parser_rs::parser::ast::classes::{ClassExtends, ClassMember, ClassStatement};
use php_parser_rs::parser::ast::constant::ConstantEntry;
use php_parser_rs::parser::ast::constant::ConstantStatement;
use php_parser_rs::parser::ast::control_flow::IfStatement;
use php_parser_rs::parser::ast::functions::{
    FunctionParameter, FunctionParameterList, MethodBody, ReturnType,
};
use php_parser_rs::parser::ast::identifiers::Identifier;
use php_parser_rs::parser::ast::identifiers::{DynamicIdentifier, SimpleIdentifier};
use php_parser_rs::parser::ast::literals::Literal;
use php_parser_rs::parser::ast::modifiers::MethodModifierGroup;
use php_parser_rs::parser::ast::modifiers::{
    MethodModifier, PropertyModifier, PropertyModifierGroup,
};
use php_parser_rs::parser::ast::operators;
use php_parser_rs::parser::ast::operators::AssignmentOperationExpression::*;
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::variables::Variable;
use php_parser_rs::parser::ast::Block;
use php_parser_rs::parser::ast::BlockStatement;
use php_parser_rs::parser::ast::ReturnStatement;
use php_parser_rs::parser::ast::Statement;
use php_parser_rs::parser::ast::{Expression, ExpressionStatement};
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
pub fn propperty_exists(identifier: php_parser_rs::parser::ast::identifiers::Identifier) -> bool {
    match identifier {
        php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(identifier) => {
            match (identifier) {
                SimpleIdentifier { span, value } => {
                    let property_value = value;
                    // for m in &file.members {
                    //     match m {
                    //         ClassMember::Property(p) => {
                    //             for entry in &p.entries {
                    //                 match (entry) {
                    //                     PropertyEntry::Uninitialized { variable } => {
                    //                         return variable.name.to_string()
                    //                             == format!("${}", property_value);
                    //                     }
                    //                     PropertyEntry::Initialized {
                    //                         variable,
                    //                         equals,
                    //                         value,
                    //                     } => {
                    //                         return variable.name.to_string()
                    //                             == format!("${}", property_value);
                    //                     }
                    //                 }
                    //             }
                    //         }
                    //         _ => {}
                    //     }
                    // }
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
/// Find a class based on the name
// pub fn find_class( fqn: &str) -> Option<ClassStatement> {
//     //todo find the class here.
//     return classes.get(fqn).cloned();
// }

/// Check if the opening tag is on the right position.
pub fn opening_tag(t: Span) -> Option<Suggestion> {
    if t.line > 1 {
        return Some(Suggestion::from(
                    "The opening tag <?php is not on the right line. This should always be the first line in a PHP file.".to_string(),
                    t
                ));
    }

    if t.column > 1 {
        return Some(Suggestion::from(
            format!(
                "The opening tag doesn't start at the right column: {}.",
                t.column
            )
            .to_string(),
            t,
        ));
    }
    None
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

/// Analyse class statement.
pub fn class_statement_analyze(class_statement: ClassStatement) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();
    let name = String::from(class_statement.name.value);
    match has_capitalized_name(name.clone(), class_statement.class) {
        Some(s) => {
            suggestions.push(s);
        }
        None => {}
    };

    for member in class_statement.body.members {
        // file.members.push(member.clone());
        class_member_analyze(member);
    }

    let extends = class_statement.extends;
    match extends {
        Some(ClassExtends { extends, parent }) => {
            // let exists = find_class(String::from(parent.value.clone()).as_str());
            // match exists {
            //     None => suggestions.push(Suggestion::from(
            //         format!(
            //             "{} is extending a class({}) that doesnt exits.",
            //             name, parent.value
            //         ),
            //         class_statement.class,
            //     )),
            //     _ => {}
            // }
        }
        None => {}
    };
    suggestions
}

/// Analyse class member.
pub fn class_member_analyze(member: ClassMember) -> Vec<Suggestion> {
    let suggestions = Vec::new();
    // match member {
    //     ClassMember::Property(property) => {
    //         let name = property_name(property.clone());
    //         if property_without_modifiers(property.clone()) {
    //             suggestions.push(Suggestion::from(
    //                 format!("The variables {} have no modifier.", name.join(", ")).to_string(),
    //                 property.end,
    //             ));
    //         }
    //     }
    //     ClassMember::Constant(constant) => {
    //         for entry in constant.entries {
    //             if uppercased_constant_name(entry.clone()) == false {
    //                 suggestions.push(Suggestion::from(
    //                     format!(
    //                         "All letters in a constant({}) should be uppercased.",
    //                         entry.name.value.to_string()
    //                     ),
    //                     entry.name.span,
    //                 ))
    //             }
    //         }
    //     }
    //     ClassMember::TraitUsage(_trait) => {}
    //     ClassMember::AbstractMethod(_) => {}
    //     ClassMember::ConcreteMethod(concretemethod) => {
    //         let method_name = concretemethod.name.value;
    //         match concretemethod.modifiers {
    //             MethodModifierGroup { modifiers } => {
    //                 if modifiers.len() == 0 {
    //                     suggestions.push(Suggestion::from(
    //                         format!("The method {} has no modifiers.", method_name).to_string(),
    //                         concretemethod.function,
    //                     ))
    //                 }
    //             }
    //         };
    //         // Detect parameters without type.
    //         match concretemethod.parameters {
    //             FunctionParameterList {
    //                 comments,
    //                 left_parenthesis,
    //                 right_parenthesis,
    //                 parameters,
    //             } => {
    //                 for parameter in parameters.inner {
    //                     if function_parameter_without_type(parameter.clone()) {
    //                         suggestions.push(Suggestion::from(
    //                             format!(
    //                                 "The parameter({}) in the method {} has no datatype.",
    //                                 parameter.name, method_name
    //                             )
    //                             .to_string(),
    //                             concretemethod.function,
    //                         ));
    //                     }
    //                 }
    //             }
    //         }
    //         match concretemethod.body.clone() {
    //             MethodBody {
    //                 comments,
    //                 left_brace,
    //                 statements,
    //                 right_brace,
    //             } => {
    //                 for statement in statements {
    //                     match statement {
    //                         Statement::Expression(ExpressionStatement { expression, ending }) => {
    //                             analyze_expression(expression);
    //                         }

    //                         Statement::Expression(ExpressionStatement { expression, ending }) => {
    //                             match expression {
    //                                 Expression::MethodCall(method_call_expression) => {
    //                                     match *method_call_expression.method {
    //                                         _ => {}
    //                                         Expression::Identifier(
    //                                             Identifier::SimpleIdentifier(s),
    //                                         ) => {
    //                                             match *method_call_expression.target {
    //                                                 Expression::Variable(
    //                                                     Variable::SimpleVariable(s),
    //                                                 ) => {
    //                                                     if s.name.to_string()
    //                                                         == String::from("$this")
    //                                                     {
    //                                                         let mut exists = false;
    //                                                         for member in file.members.iter() {
    //                                                             match member.clone() {
    //                                                                 ClassMember::ConcreteMethod(
    //                                                                     ConcreteMethod {
    //                                                                         comments,
    //                                                                         attributes,
    //                                                                         modifiers,
    //                                                                         function,
    //                                                                         ampersand,
    //                                                                         name,
    //                                                                         parameters,
    //                                                                         return_type,
    //                                                                         body,
    //                                                                     },
    //                                                                 ) => {
    //                                                                     if exists == false
    //                                                                         && name.to_string()
    //                                                                             == String::from(
    //                                                                                 s.name.clone(),
    //                                                                             )
    //                                                                     {
    //                                                                         exists = true;
    //                                                                     }
    //                                                                 }
    //                                                                 _ => {}
    //                                                             };
    //                                                         }
    //                                                         if exists == false {
    //                                                             let suggestion = Suggestion::from(
    //                                                                                 format!(
    //                                                                                     "The method {} is being called but it doesn't exists. ",
    //                                                                                     String::from(s.name)
    //                                                                                     ),
    //                                                                                 s.span);
    //                                                             suggestions.push(suggestion);
    //                                                         };
    //                                                     };
    //                                                 }
    //                                                 _ => {}
    //                                             };
    //                                         }
    //                                     };
    //                                 }
    //                                 _ => {}
    //                             }
    //                         }
    //                         _ => {}
    //                     };
    //                 }
    //             }
    //         };

    //         // Detect return statement without the proper return type signature.
    //         let has_return = method_has_return(concretemethod.body.clone());

    //         match has_return {
    //             Some(ReturnStatement {
    //                 r#return,
    //                 value,
    //                 ending,
    //             }) => {
    //                 match concretemethod.return_type {
    //                     None => {
    //                         suggestions.push(
    //                                                             Suggestion::from(
    //                                                                 format!("The {} has a return statement but it has no return type signature.", method_name).to_string(),
    //                                                             r#return
    //                                                             )
    //                                                         );
    //                     }
    //                     _ => {}
    //                 };
    //             }
    //             None => {}
    //         };
    //     }
    //     ClassMember::VariableProperty(_) => {}
    //     ClassMember::AbstractConstructor(_constructor) => {}
    //     ClassMember::ConcreteConstructor(constructor) => {
    //         for statement in constructor.body.statements {
    //             match statement {
    //                 Statement::Expression(ExpressionStatement { expression, ending }) => {
    //                     analyze_expression(expression);
    //                 }

    //                 _ => {}
    //             }
    //         }
    //     }
    // }
    suggestions
}

///
fn property_name(property: Property) -> Vec<std::string::String> {
    return match property {
        Property {
            attributes,
            modifiers,
            r#type,
            entries,
            end,
        } => {
            let mut names: Vec<String> = Vec::new();
            for entry in entries {
                let name = match entry {
                    PropertyEntry::Initialized {
                        variable,
                        equals,
                        value,
                    } => variable.name.to_string(),
                    PropertyEntry::Uninitialized { variable } => variable.to_string(),
                };
                names.push(name);
            }
            return names;
        }
    };
}
/// Analyze expressions.
pub fn analyze_expression(expresion: Expression) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();
    match expresion {
        Expression::Cast(_) => {}
        Expression::YieldFrom(_) => {}
        Expression::Yield(_) => {}
        Expression::Match(_) => {}
        Expression::Throw(_) => {}
        Expression::Clone(_) => {}
        Expression::Coalesce(_) => {}
        Expression::Ternary(_) => {}
        Expression::Null => {}
        Expression::MagicConstant(_) => {}
        Expression::Bool(_) => {}
        Expression::AnonymousClass(_) => {}
        Expression::Nowdoc(_) => {}
        Expression::Heredoc(_) => {}
        Expression::ArrowFunction(_) => {}
        Expression::Closure(_) => {}
        Expression::List(_) => {}
        Expression::Array(_) => {}
        Expression::Parent => {}
        Expression::ShortArray(_) => {}
        Expression::Self_ => {}
        Expression::Static => {}
        Expression::ConstantFetch(_) => {}
        Expression::StaticPropertyFetch(_) => {}
        Expression::NullsafePropertyFetch(_) => {}
        Expression::NullsafeMethodCall(_) => {}
        Expression::PropertyFetch(property) => match *property.target {
            Expression::Variable(v) => match v {
                Variable::BracedVariableVariable(_) => {}
                Variable::SimpleVariable(variable) => {
                    if variable.name.to_string() == String::from("$this") {
                        let identifier = *property.property;

                        match identifier {
                            Expression::Identifier(identifier) => {
                                let exists = propperty_exists(identifier.clone());
                                let name = get_property_name(identifier.clone());
                                let span: Span = match identifier {
                    php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(
                        identifier,
                    ) => Some(identifier.span),
                    _ => Some(Span { line: 0, column: 0, position: 0 })   // todo fix this
                                            ,
                }.unwrap();
                                if exists == false {
                                    suggestions.push(Suggestion::from(
                                        format!(
                            "The property {} is being called, but it does not exists.",
                            name
                        )
                                        .to_string(),
                                        span,
                                    ));
                                }
                            }

                            _ => {}
                        }
                    }
                }
                Variable::VariableVariable(_) => {}
            },

            __ => {}
        },
        Expression::StaticMethodClosureCreation(_) => {}
        Expression::StaticVariableMethodClosureCreation(_) => {}
        Expression::StaticVariableMethodCall(_) => {}
        Expression::StaticMethodCall(_) => {}
        Expression::MethodCall(_) => {}
        Expression::FunctionCall(_) => {}
        Expression::RequireOnce(_) => {}
        Expression::Require(_) => {}
        Expression::Include(_) => {}
        Expression::Variable(_) => {}
        Expression::Identifier(identifier) => {
            let exists = propperty_exists(identifier.clone());
            let name = get_property_name(identifier.clone());
            let span: Span = match identifier {
                php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(
                    identifier,
                ) => identifier.span,
                _ => todo!(),
            };
            if exists == false {
                suggestions.push(Suggestion::from(
                    format!(
                        "The property {} is being called, but it does not exists.",
                        name
                    )
                    .to_string(),
                    span,
                ));
            }
        }
        Expression::Instanceof(_) => {}
        Expression::Concat(_) => {}
        Expression::ArithmeticOperation(_) => {}
        Expression::Literal(_) => {}
        Expression::Print(_) => {}
        Expression::Unset(_) => {}
        Expression::Isset(_) => {}
        Expression::Empty(_) => {}
        Expression::AssignmentOperation(assignment) => match assignment {
            Assign {
                left,
                equals,
                right,
            } => {
                suggestions.append(&mut analyze_expression(*left));
            }
            _ => {}
        },
        Expression::Eval(_) => {}
        Expression::Die(_) => {}
        Expression::Noop {} => {}
        Expression::New(_) => {}
        Expression::Exit(_) => {}
        Expression::StaticVariableMethodClosureCreation(_) => {}
        Expression::StaticVariableMethodCall(_) => {}
        Expression::MethodClosureCreation(_) => {}
        Expression::AssignmentOperation(_) => {}
        Expression::FunctionClosureCreation(_) => {}
        Expression::InterpolatedString(_) => {}
        Expression::LogicalOperation(operation) => {}
        Expression::BitwiseOperation(operation) => {}
        Expression::NullsafeMethodCall(_) => {}
        Expression::ErrorSuppress(_) => {}
        Expression::IncludeOnce(_) => {}
        Expression::ShellExec(_) => {}
        Expression::Require(_) => {}
        Expression::ComparisonOperation(operation) => {}
        Expression::Parenthesized(_) => {}
        Expression::ArrayIndex(_) => {}
        Expression::ShortTernary(_) => {}
        Expression::Reference(_) => {}
    };
    suggestions
}

pub struct Analyse {}

impl Analyse {
    pub fn statement(statement: parser::ast::Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        match statement {
            Statement::FullOpeningTag(tag) => {
                let s = opening_tag(tag.span);
                suggestions.push(s.unwrap())
            }

            Statement::ShortOpeningTag(tag) => {
                let s = opening_tag(tag.span).unwrap();
                suggestions.push(s);
            }
            Statement::EchoOpeningTag(_) => {}
            Statement::ClosingTag(_) => {}
            Statement::InlineHtml(_) => {}
            Statement::Label(_) => {}
            Statement::Goto(_) => {}
            Statement::HaltCompiler(_) => {}
            Statement::Static(_) => {}
            Statement::DoWhile(_) => {}
            Statement::While(_) => {}
            Statement::For(_) => {}
            Statement::Foreach(_) => {}
            Statement::Break(_) => {}
            Statement::Continue(_) => {}
            Statement::Constant(_) => {}
            Statement::Function(_) => {}
            Statement::Class(class_statement) => {
                suggestions.append(&mut class_statement_analyze(class_statement));
            }
            Statement::Trait(_) => {}
            Statement::Interface(_) => {}
            Statement::If(_) => {}
            Statement::Switch(_) => {}
            Statement::Echo(_) => {}
            Statement::Expression(expression_statement) => {
                suggestions.append(&mut analyze_expression(expression_statement.expression));
            }
            Statement::Return(_) => {
                // println!("Hello world");
                // println!("{return_statement:#?}")
            }
            Statement::Namespace(namespace) => match namespace {
                parser::ast::namespaces::NamespaceStatement::Unbraced(unbraced) => {
                    for statement in unbraced.statements {
                        match statement {
                            Statement::Class(class_statement) => {
                                suggestions.append(&mut class_statement_analyze(class_statement));
                            }
                            _ => {}
                        }
                    }
                }
                parser::ast::namespaces::NamespaceStatement::Braced(braced) => {
                    for statement in braced.body.statements {
                        match statement {
                            Statement::Class(class_statement) => {
                                suggestions.append(&mut class_statement_analyze(class_statement));
                            }
                            _ => {}
                        }
                    }
                }
            },
            Statement::Use(_) => {}
            Statement::GroupUse(_) => {}
            Statement::Comment(_) => {}
            Statement::Try(_) => {}
            Statement::UnitEnum(_) => {}
            Statement::BackedEnum(_) => {}
            Statement::Block(_) => {}
            Statement::Global(_) => {}
            Statement::Declare(_) => {}
            Statement::Noop(_) => {}
        };

        suggestions
    }
}
