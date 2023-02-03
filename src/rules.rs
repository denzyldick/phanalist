use colored::*;
use php_parser_rs::lexer::byte_string::ByteString;
use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::classes::{ClassExtends, ClassMember, ClassStatement};
use php_parser_rs::parser::ast::constant::{ClassishConstant, ConstantEntry};
use php_parser_rs::parser::ast::functions::{
    FunctionParameter, FunctionParameterList, MethodBody, ReturnType,
};
use std::io::Error;
use walkdir::WalkDir;

use php_parser_rs::parser;
use php_parser_rs::parser::ast::identifiers::{DynamicIdentifier, Identifier, SimpleIdentifier};
use php_parser_rs::parser::ast::modifiers::{
    MethodModifier, MethodModifierGroup, PropertyModifier, PropertyModifierGroup,
};
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::variables::SimpleVariable;
use php_parser_rs::parser::ast::Expression;
use php_parser_rs::parser::ast::{operators, ReturnStatement};
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;
use std::convert::identity;
use std::io::Read;
use std::path::PathBuf;
use std::{env, fs};

use php_parser_rs::parser::ast::Statement;

use crate::analyse::{self, *};

#[derive(Debug, Clone)]
pub struct Project {
    pub files: Vec<File>,
    pub classes: HashMap<String, ClassStatement>,
}

impl Project {
    pub fn scan_folder(&mut self, current_dir: PathBuf) {
        for entry in WalkDir::new(current_dir) {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = fs::metadata(&path).unwrap();
            let file_name = match path.file_name() {
                Some(f) => String::from(f.to_str().unwrap()),
                None => String::from(""),
            };
            if file_name != "." || file_name != "" {
                if metadata.is_file() {
                    if let Some(extension) = path.extension() {
                        if extension == "php" {
                            let content = fs::read_to_string(entry.path());
                            match content {
                                Err(err) => {
                                    println!("{:?}", err);
                                }
                                Ok(content) => {
                                    for statement in self.parse_code(content.as_str()) {
                                        let mut file = File {
                                            path: entry.path().to_path_buf(),
                                            ast: Some(statement.clone()),
                                            members: Vec::new(),
                                            suggestions: Vec::new(),
                                        };

                                        self.files.push(file);
                                    }
                                }
                            }
                        }
                    } else if metadata.is_dir() {
                        self.scan_folder(path.to_path_buf());
                    }
                }
            }
        }
    }

    fn parse_code(&self, code: &str) -> Vec<php_parser_rs::parser::ast::Statement> {
        match parser::parse(code) {
            Ok(ast) => ast,
            Err(err) => {
                // println!("{:#?}", err);
                vec![]
            }
        }
    }
    fn build_class_list(mut self) -> Project {
        for mut file in self.files.clone() {
            let ast = match file.start() {
                None => {}
                Some(a) => {
                    let k = String::from(a.name.value.clone());
                    let v = a.clone();
                    self.classes.insert(k, v);
                }
            };
        }
        self
    }
    fn run(&mut self) {
        let mut s = self;
        let files = &mut s.clone().files;
        for i in files.iter() {
            let f = &mut i.clone();
            s.analyze(f);
        }
    }
    pub fn start(self) -> Result<String, Error> {
        let mut s = self;
        s = s.build_class_list();
        s.run();

        Ok("".to_string())
    }
    pub fn add(&mut self, file: File) {
        self.files.push(file)
    }

    pub fn find_class(&self, fqn: &str) -> Option<ClassStatement> {
        return self.classes.get(fqn).cloned();
    }

    pub fn opening_tag(&mut self, t: Span, file: &mut File) -> &mut Project {
        if t.line > 1 {
            file.suggestions.push(
                Suggestion::from(
                    "The opening tag <?php is not on the right line. This should always be the first line in a PHP file.".to_string(),
                    t
                ))
        }

        if t.column > 1 {
            file.suggestions.push(Suggestion::from(
                format!(
                    "The opening tag doesn't start at the right column: {}.",
                    t.column
                )
                .to_string(),
                t,
            ));
        }
        self
    }
    pub fn analyze(&mut self, file: &mut File) -> &mut Project {
        let statement = file.ast.clone().unwrap();
        let mut project = self;
        match statement {
            Statement::FullOpeningTag(span) => project = project.opening_tag(span, file),
            Statement::ShortOpeningTag(span) => project = project.opening_tag(span, file),
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
                // println!("{:?}", String::from(ClassStatement.name.value.clone()),);
                let name = String::from(ClassStatement.name.value);
                match analyse::has_capitalized_name(name.clone(), ClassStatement.class) {
                    Some(s) => {
                        file.suggestions.push(s);
                    }
                    None => {}
                }

                for member in ClassStatement.body.members {
                    file.members.push(member.clone());
                    project = project.class_member_analyze(member, file);
                }
                let extends = ClassStatement.extends;
                match extends {
                    Some(ClassExtends { extends, parent }) => {
                        let exists =
                            project.find_class(String::from(parent.value.clone()).as_str());
                        match exists {
                            None => file.suggestions.push(Suggestion::from(
                                format!(
                                    "{} is extending a class({}) that doesnt exits.",
                                    name, parent.value
                                ),
                                ClassStatement.class,
                            )),
                            _ => {}
                        }
                    }
                    None => {}
                };
            }
            Statement::Trait(_TraitStatement) => {}
            Statement::Interface(_InterfaceStatement) => {}
            Statement::If(_IfStatement) => {}
            Statement::Switch(_SwitchStatement) => {}
            Statement::Echo(_EchoStatement) => {}
            Statement::Expression(ExpressionStatement) => {
                project = project.analyze_expression(ExpressionStatement.expression, file.clone())
            }
            Statement::Return(_ReturnStatement) => {}
            Statement::Namespace(namespace) => match namespace {
                parser::ast::namespaces::NamespaceStatement::Unbraced(unbraced) => {
                    for statement in unbraced.statements {
                        match statement {
                            Statement::Class(ClassStatement) => {
                                project.class_statement_analyze(ClassStatement, file);
                            }
                            _ => {}
                        }
                    }
                }
                parser::ast::namespaces::NamespaceStatement::Braced(braced) => {
                    for statement in braced.body.statements {
                        match statement {
                            Statement::Class(ClassStatement) => {
                                project.class_statement_analyze(ClassStatement, file);
                            }
                            _ => {}
                        }
                    }
                }
            },
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
        file.output(Output::STDOUT);
        project
    }

    pub fn class_statement_analyze(
        &mut self,
        ClassStatement: ClassStatement,
        file: &mut File,
    ) -> &mut Project {
        let mut project = self;
        let name = String::from(ClassStatement.name.value);
        match analyse::has_capitalized_name(name.clone(), ClassStatement.class) {
            Some(s) => {
                file.suggestions.push(s);
            }
            None => {}
        }

        for member in ClassStatement.body.members {
            file.members.push(member.clone());
            project = project.class_member_analyze(member, file);
        }
        let extends = ClassStatement.extends;
        match extends {
            Some(ClassExtends { extends, parent }) => {
                let exists = project.find_class(String::from(parent.value.clone()).as_str());
                match exists {
                    None => file.suggestions.push(Suggestion::from(
                        format!(
                            "{} is extending a class({}) that doesnt exits.",
                            name, parent.value
                        ),
                        ClassStatement.class,
                    )),
                    _ => {}
                }
            }
            None => {}
        };
        project
    }
    pub fn class_member_analyze(&mut self, member: ClassMember, file: &mut File) -> &mut Project {
        match member {
            ClassMember::Property(property) => {
                let name = self.property_name(property.clone());
                if analyse::property_without_modifiers(property.clone()) {
                    file.suggestions.push(Suggestion::from(
                        format!("The variables {} have no modifier.", name.join(", ")).to_string(),
                        property.end,
                    ));
                }
                self
            }
            ClassMember::Constant(constant) => {
                for entry in constant.entries {
                    if (analyse::uppercased_constant_name(entry.clone()) == false) {
                        file.suggestions.push(Suggestion::from(
                            format!(
                                "All letters in a constant({}) should be uppercased.",
                                entry.name.value.to_string()
                            ),
                            entry.name.span,
                        ))
                    }
                }
                self
            }
            ClassMember::TraitUsage(_trait) => self,
            ClassMember::AbstractMethod(abstractmethod) => self,
            ClassMember::ConcreteMethod(concretemethod) => {
                let method_name = concretemethod.name.value;
                match concretemethod.modifiers {
                    MethodModifierGroup { modifiers } => {
                        if modifiers.len() == 0 {
                            file.suggestions.push(Suggestion::from(
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
                            if analyse::function_parameter_without_type(parameter.clone()) {
                                file.suggestions.push(Suggestion::from(
                                    format!(
                                        "The parameter({}) in the method {} has no datatype.",
                                        parameter.name, method_name
                                    )
                                    .to_string(),
                                    concretemethod.function,
                                ));
                            }
                        }
                    }
                }

                // Detect return statement without the proper return type signature.
                //
                let has_return = analyse::method_has_return(concretemethod.body);

                match has_return {
                    Some(ReturnStatement {
                        r#return,
                        value,
                        ending,
                    }) => {
                        match concretemethod.return_type {
                            None => {
                                file.suggestions.push(
                                                                Suggestion::from(
                                                                    format!("The {} has a return statement but it has no return type signature.", method_name).to_string(),
                                                                r#return
                                                                )
                                                            );
                            }
                            _ => {}
                        };
                    }
                    None => {}
                };
                self
            }
            ClassMember::VariableProperty(variableproperty) => self,
            ClassMember::AbstractConstructor(_constructor) => self,
            ClassMember::ConcreteConstructor(constructor) => {
                for statement in constructor.body.statements {
                    let f = &mut file.clone();
                }
                self
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

    pub fn analyze_expression(&mut self, expresion: Expression, mut file: File) -> &mut Project {
        let mut project = self;
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
                project = project.analyze_expression(*property, file);
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
                let exists = analyse::propperty_exists(identifier.clone(), file.clone());
                let name = analyse::get_property_name(identifier.clone());
                let span: Span = match identifier {
                    php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(
                        identifier,
                    ) => identifier.span,
                    _ => todo!(),
                };
                if exists == false {
                    file.suggestions.push(Suggestion::from(
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
                } => project = project.analyze_expression(*left, file),
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
        project
    }
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
#[derive(Debug, Clone)]
pub struct File {
    pub path: PathBuf,
    pub ast: Option<Statement>,
    pub members: Vec<ClassMember>,
    pub suggestions: Vec<Suggestion>,
}
#[derive(Debug)]
pub enum Output {
    STDOUT,
    FILE,
}
impl File {
    pub fn output(&mut self, location: Output) {
        match location {
            Output::STDOUT => {
                if self.suggestions.len() > 0 {
                    let file_symbol = "--->".blue().bold();
                    println!("{} {} ", file_symbol, self.path.display());
                    println!(
                        "{} {}",
                        "Warnings detected: ".yellow().bold(),
                        self.suggestions.len().to_string().as_str().red().bold()
                    );
                    let line_symbol = "|".blue().bold();
                    println!("  \t{}", line_symbol);
                    for suggestion in &self.suggestions {
                        println!(
                            "  {}\t{} {}",
                            format!("{}:{}", suggestion.span.line, suggestion.span.column)
                                .blue()
                                .bold(),
                            line_symbol,
                            suggestion.suggestion
                        );
                    }
                    println!("  \t{}", line_symbol);
                    println!("")
                }
            }
            Output::FILE => {}
        }
    }
    pub fn start(&mut self) -> Option<php_parser_rs::parser::ast::classes::ClassStatement> {
        return match (self.ast.to_owned().unwrap()) {
            Statement::Class(c) => Some(c),
            _ => None,
        };
    }
}
