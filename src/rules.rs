use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::functions::{
    FunctionParameter, FunctionParameterList, MethodBody, ReturnType,
};
use php_parser_rs::parser::ast::identifiers::{DynamicIdentifier, Identifier, SimpleIdentifier};
use php_parser_rs::parser::ast::modifiers::{
    MethodModifier, MethodModifierGroup, PropertyModifier, PropertyModifierGroup,
};
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::variables::SimpleVariable;
use php_parser_rs::parser::ast::Expression;
use php_parser_rs::parser::ast::{operators, ReturnStatement};
use std::convert::identity;
use std::path::PathBuf;

use php_parser_rs::parser::ast::Statement;
#[derive(Debug, Clone)]
pub struct File {
    pub path: PathBuf,
    pub ast: Option<Statement>,
    pub members: Vec<ClassMember>,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    suggestion: String,
    span: Span,
}

impl Suggestion {
    pub fn from(suggesion: String, span: Span) -> Self {
        Self {
            suggestion: suggesion,
            span: span,
        }
    }
}

pub enum Output {
    STDOUT,
    FILE,
}
impl File {
    pub fn output(&mut self, location: Output) {
        match location {
            Output::STDOUT => {
                if self.suggestions.len() > 0 {
                    println!("{} ", self.path.display());
                    println!("Found {} suggestions detected. ", self.suggestions.len());
                    for suggestion in &self.suggestions {
                        println!("Line: {} - {}", suggestion.span.line, suggestion.suggestion);
                    }
                    println!("");
                }
            }
            Output::FILE => {}
        }
    }

    pub fn opening_tag(&mut self, t: Span) {
        if t.line > 1 {
            self.suggestions.push(
                Suggestion::from(
                    "The opening tag <?php is not on the right line. This should always be the first line in a PHP file.".to_string(),
                    t
                ))
        }

        if t.column > 1 {
            self.suggestions.push(Suggestion::from(
                format!(
                    "The opening tag doesn't start at the right column: {}.",
                    t.column
                )
                .to_string(),
                t,
            ));
        }
    }
    pub fn start(&mut self) {
        match self.ast {
            Some(ref ast) => self.analyze(ast.to_owned()),
            None => {
                println!("No generated");
            }
        }
    }
    pub fn analyze(&mut self, statement: php_parser_rs::parser::ast::Statement) {
        match statement {
            Statement::FullOpeningTag(span) => self.opening_tag(span),
            Statement::FullOpeningTag(span) => self.opening_tag(span),
            Statement::ShortOpeningTag(span) => self.opening_tag(span),
            Statement::EchoOpeningTag(_Span) => {}
            Statement::ClosingTag(_Span) => {}
            Statement::InlineHtml(_ByteString) => {}
            Statement::Label(_LabelStatement) => {}
            Statement::Goto(_GotoStatement) => {}
            Statement::HaltCompiler(_HaltCompiler) => {}
            Statement::Static(_StaticStatement) => {}
            Statement::DoWhile(_DoWhileStatement) => {}
            Statement::While(_WhileStatement) => {}
            Statement::For(_ForStatement) => {}
            Statement::Foreach(_ForeachStatement) => {}
            Statement::Break(_BreakStatement) => {}
            Statement::Continue(_ContinueStatement) => {}
            Statement::Constant(_ConstantStatement) => {}
            Statement::Function(_FunctionStatement) => {}
            Statement::Class(ClassStatement) => {
                let name = String::from(ClassStatement.name.value);
                self.has_capitalized_name(name, ClassStatement.class);
                for member in ClassStatement.body.members {
                    self.members.push(member.clone());
                    self.class_member_analyze(member);
                }
                let extends = ClassStatement.extends;

                match extends {
                    Some(e) => {}
                    None => {}
                }
            }
            Statement::Trait(_TraitStatement) => {}
            Statement::Interface(_InterfaceStatement) => {}
            Statement::If(_IfStatement) => {}
            Statement::Switch(_SwitchStatement) => {}
            Statement::Echo(_EchoStatement) => {}
            Statement::Expression(ExpressionStatement) => {
                self.analyze_expression(ExpressionStatement.expression)
            }
            Statement::Return(_ReturnStatement) => {}
            Statement::Namespace(_NamespaceStatement) => {}
            Statement::Use(_UseStatement) => {}
            Statement::GroupUse(_GroupUseStatement) => {}
            Statement::Comment(_Comment) => {}
            Statement::Try(_TryStatement) => {}
            Statement::UnitEnum(_UnitEnumStatement) => {}
            Statement::BackedEnum(_BackedEnumStatement) => {}
            Statement::Block(_BlockStatement) => {}
            Statement::Global(_GlobalStatement) => {}
            Statement::Declare(_DeclareStatement) => {}
            Statement::Noop(_Span) => {}
        }
    }

    pub fn has_capitalized_name(&mut self, name: String, span: Span) {
        if !name.chars().next().unwrap().is_uppercase() {
            self.suggestions.push(Suggestion::from(
                format!("The class name {} is not capitlized. The first letter of the name of the class should be in uppercase.", name).to_string(),
                span
            ))
        }
    }

    pub fn class_member_analyze(&mut self, member: ClassMember) {
        match member {
            ClassMember::Property(property) => {
                let name = self.property_name(property.clone());
                match property.modifiers {
                    PropertyModifierGroup { modifiers } => {
                        if modifiers.len() == 0 {
                            self.suggestions.push(Suggestion::from(
                                format!("The variables {} have no modifier.", name.join(", "))
                                    .to_string(),
                                property.end,
                            ));
                        }
                    }
                }
            }
            ClassMember::Constant(_constant) => {}
            ClassMember::TraitUsage(_trait) => {}
            ClassMember::AbstractMethod(abstractmethod) => {}
            ClassMember::ConcreteMethod(concretemethod) => {
                let method_name = concretemethod.name.value;
                match concretemethod.modifiers {
                    MethodModifierGroup { modifiers } => {
                        if modifiers.len() == 0 {
                            self.suggestions.push(Suggestion::from(
                                format!("The method {} has no modifiers.", method_name).to_string(),
                                concretemethod.function,
                            ))
                        }
                    }
                };
                // Detect parameters without type.
                match concretemethod.parameters {
                    FunctionParameterList {
                        comments,
                        left_parenthesis,
                        right_parenthesis,
                        parameters,
                    } => {
                        for parameter in parameters.inner {
                            match parameter {
                                FunctionParameter {
                                    comments,
                                    name,
                                    attributes,
                                    data_type,
                                    ellipsis,
                                    default,
                                    ampersand,
                                } => {
                                    match data_type {
                                        None => {
                                            self.suggestions.push(
                                                        Suggestion::from(
                                                            format!("The parameter({}) in the method {} has no datatype.", name, method_name).to_string(),
                                                            concretemethod.function
                                                        )
                                                    );
                                        }
                                        Some(_) => {}
                                    }
                                }
                            }
                        }
                    }
                }

                // Detect return statement without the proper return type signature.
                match concretemethod.body {
                    MethodBody {
                        comments,
                        left_brace,
                        statements,
                        right_brace,
                    } => {
                        for statement in statements {
                            let i = match statement {
                                Statement::Return(ReturnStatement {
                                    r#return,
                                    value,
                                    ending,
                                }) => match value {
                                    None => None,
                                    Some(s) => match s {
                                        Expression::Literal(l) => {
                                            match concretemethod.return_type {
                                                None => {
                                                    self.suggestions.push(
                                                                Suggestion::from(
                                                                    format!("The {} has a return statement but it has no return type signature.", method_name).to_string(),
                                                                r#return
                                                                )
                                                            );
                                                }
                                                Some(_) => {}
                                            }
                                            Some(l)
                                        }
                                        _ => None,
                                    },
                                },
                                _ => None,
                            };
                        }
                    }
                }
            }
            ClassMember::VariableProperty(variableproperty) => {}
            ClassMember::AbstractConstructor(_constructor) => {}
            ClassMember::ConcreteConstructor(constructor) => {
                for statement in constructor.body.statements {
                    self.analyze(statement);
                }
            }
        }
    }

    fn property_name(&self, property: Property) -> Vec<std::string::String> {
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

    fn propperty_exists(
        &self,
        identifier: php_parser_rs::parser::ast::identifiers::Identifier,
    ) -> bool {
        match identifier {
            php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(identifier) => {
                match (identifier) {
                    SimpleIdentifier { span, value } => {
                        let property_value = value;
                        for m in &self.members {
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

    pub fn analyze_expression(&mut self, expresion: Expression) {
        match expresion {
            Expression::Cast { cast, kind, value } => {}
            Expression::YieldFrom { value } => {}
            Expression::Yield { key, value } => {}
            Expression::Match {
                keyword,
                left_parenthesis,
                condition,
                right_parenthesis,
                left_brace,
                default,
                arms,
                right_brace,
            } => {}
            Expression::Throw { value } => {}
            Expression::Clone { target } => {}
            Expression::Coalesce {
                lhs,
                double_question,
                rhs,
            } => {}
            Expression::Ternary {
                condition,
                question,
                then,
                colon,
                r#else,
            } => {}
            Expression::Null => {}
            Expression::MagicConstant(constant) => {}
            Expression::Bool { value } => {}
            Expression::AnonymousClass(class) => {}
            Expression::Nowdoc { value } => {}
            Expression::Heredoc { parts } => {}
            Expression::ArrowFunction(function) => {}
            Expression::Closure(closure) => {}
            Expression::List {
                list,
                start,
                items,
                end,
            } => {}
            Expression::Array {
                array,
                start,
                items,
                end,
            } => {}
            Expression::Parent => {}
            Expression::ShortArray { start, items, end } => {}
            Expression::Self_ => {}
            Expression::Static => {}
            Expression::ConstantFetch {
                target,
                double_colon,
                constant,
            } => {}
            Expression::StaticPropertyFetch {
                target,
                double_colon,
                property,
            } => {}
            Expression::NullsafePropertyFetch {
                target,
                question_arrow,
                property,
            } => {}
            Expression::NullsafeMethodCall {
                target,
                question_arrow,
                method,
                arguments,
            } => {}
            Expression::PropertyFetch {
                target,
                arrow,
                property,
            } => {
                self.analyze_expression(*property);
            }
            Expression::StaticMethodClosureCreation {
                target,
                double_colon,
                method,
                placeholder,
            } => {}
            Expression::StaticVariableMethodClosureCreation {
                target,
                double_colon,
                method,
                placeholder,
            } => {}
            Expression::StaticVariableMethodCall {
                target,
                double_colon,
                method,
                arguments,
            } => {}
            Expression::StaticMethodCall {
                target,
                double_colon,
                method,
                arguments,
            } => {}
            Expression::MethodCall {
                target,
                arrow,
                method,
                arguments,
            } => {}
            Expression::FunctionCall { target, arguments } => {}
            Expression::RequireOnce { require_once, path } => {}
            Expression::Require { require, path } => {}
            Expression::Include { include, path } => {}
            Expression::Variable(variable) => {}
            Expression::Identifier(identifier) => {
                let exists = self.propperty_exists(identifier.clone());

                let name: String = match identifier.clone() {
                    php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(
                        identifier,
                    ) => identifier.value.to_string(),
                    _ => "".to_string(),
                };

                let span: Span = match identifier {
                    php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(
                        identifier,
                    ) => identifier.span,
                    _ => todo!(),
                };
                if exists == false {
                    self.suggestions.push(Suggestion::from(
                        format!(
                            "The property {} is being called, but it does not exists.",
                            name
                        )
                        .to_string(),
                        span,
                    ));
                }
            }
            Expression::Instanceof {
                left,
                instanceof,
                right,
            } => {}
            Expression::Concat { left, dot, right } => {}
            Expression::ArithmeticOperation(operation) => {}
            Expression::Literal(literal) => {}
            Expression::Print {
                print,
                value,
                argument,
            } => {}
            Expression::Unset { unset, arguments } => {}
            Expression::Isset { isset, arguments } => {}
            Expression::Empty { empty, argument } => {}
            Expression::AssignmentOperation(assignment) => match assignment {
                operators::AssignmentOperation::RightShift {
                    left,
                    right_shift_equals,
                    right,
                } => {}
                operators::AssignmentOperation::BitwiseXor {
                    left,
                    caret_equals,
                    right,
                } => {}
                operators::AssignmentOperation::BitwiseAnd {
                    left,
                    ampersand_equals,
                    right,
                } => {}
                operators::AssignmentOperation::BitwiseOr {
                    left,
                    pipe_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Concat {
                    left,
                    dot_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Subtraction {
                    left,
                    minus_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Exponentiation {
                    left,
                    pow_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Multiplication {
                    left,
                    asterisk_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Division {
                    left,
                    slash_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Assign {
                    left,
                    equals,
                    right,
                } => self.analyze_expression(*left),
                operators::AssignmentOperation::LeftShift {
                    left,
                    left_shift_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Modulo {
                    left,
                    percent_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Addition {
                    left,
                    plus_equals,
                    right,
                } => {}
                operators::AssignmentOperation::BitwiseOr {
                    left,
                    pipe_equals,
                    right,
                } => {}
                operators::AssignmentOperation::Coalesce {
                    left,
                    coalesce_equals,
                    right,
                } => {}
            },
            Expression::Eval { eval, argument } => {}
            Expression::Die { die, argument } => {}
            Expression::Noop {} => {}
            Expression::New {
                new,
                target,
                arguments,
            } => {}
            Expression::Exit { exit, argument } => {}
            Expression::StaticVariableMethodClosureCreation {
                target,
                double_colon,
                method,
                placeholder,
            } => {}
            Expression::StaticVariableMethodCall {
                target,
                double_colon,
                method,
                arguments,
            } => {}
            Expression::MethodClosureCreation {
                target,
                arrow,
                method,
                placeholder,
            } => {}
            Expression::AssignmentOperation(asignment) => {}
            Expression::FunctionClosureCreation {
                target,
                placeholder,
            } => {}
            Expression::InterpolatedString { parts } => {}
            Expression::LogicalOperation(operation) => {}
            Expression::BitwiseOperation(operation) => {}
            Expression::NullsafeMethodCall {
                target,
                question_arrow,
                method,
                arguments,
            } => {}
            Expression::ErrorSuppress { at, expr } => {}
            Expression::IncludeOnce { include_once, path } => {}
            Expression::ShellExec { parts } => {}
            Expression::Require { require, path } => {}
            Expression::ComparisonOperation(operation) => {}
            Expression::Parenthesized { start, expr, end } => {}
            Expression::ArrayIndex {
                array,
                left_bracket,
                index,
                right_bracket,
            } => {}
            Expression::ShortTernary {
                condition,
                question_colon,
                r#else,
            } => {}
            Expression::Reference { ampersand, right } => {}
        }
    }
}
