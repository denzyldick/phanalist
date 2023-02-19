use colored::*;
use php_parser_rs::lexer::byte_string::ByteString;
use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::classes::{ClassExtends, ClassMember, ClassStatement};
use php_parser_rs::parser::ast::constant::{ClassishConstant, ConstantEntry};
use php_parser_rs::parser::ast::functions::{
    FunctionParameter, FunctionParameterList, MethodBody, ReturnType,
};
use php_parser_rs::parser::error::ParseErrorStack;
use rocksdb::{IteratorMode, DB};
use std::io::Error;
use std::ops::BitXorAssign;
use std::sync::mpsc::{Receiver, Sender};

use jwalk::WalkDir;
use php_parser_rs::parser;
use php_parser_rs::parser::ast::identifiers::{DynamicIdentifier, Identifier, SimpleIdentifier};
use php_parser_rs::parser::ast::modifiers::{
    MethodModifier, MethodModifierGroup, PropertyModifier, PropertyModifierGroup,
};
use php_parser_rs::parser::ast::operators::AssignmentOperationExpression::*;
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::variables::{SimpleVariable, Variable, VariableVariable};
use php_parser_rs::parser::ast::{operators, ReturnStatement};
use php_parser_rs::parser::ast::{Expression, ExpressionStatement};
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

// Scan a directory and find all php files. When a
// file has been found the content of the file will be sent to
// as a message to the receiver.
pub fn scan_folder(current_dir: PathBuf, sender: Sender<(String, PathBuf)>) {
    for entry in WalkDir::new(current_dir.clone()).follow_links(false) {
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
                                // println!("{err:?}");
                            }
                            Ok(content) => {
                                sender.send((content, path));
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Project {
    pub fn push(mut self, file: File) -> Self {
        self.files.push(file);
        self
    }

    /// Get all files.
    pub fn get_files(self) -> Vec<File> {
        self.files
    }

    /// Build the class list.
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

    /// Iterate over the list of files and analyse the code.
    fn run(&mut self, db: &DB) {
        let mut s = self;
        let mut iter = db.iterator(IteratorMode::Start);
        while let i = iter.next().unwrap() {
            let item = i.unwrap();
            let file = item.1;
            let key = item.0;
            let path = std::str::from_utf8(&key).unwrap();
            match serde_json::from_slice(&file) {
                Err(_) => {}
                Ok(mut f) => {
                    s.analyze(&mut f);
                }
            };
        }
    }

    /// Build the class list and run analyse.
    pub fn start(self, db: &DB) -> Result<String, Error> {
        let mut s = self;

        s.run(db);

        Ok("".to_string())
    }

    /// Add a file to the files.
    pub fn add(&mut self, file: File) {
        self.files.push(file)
    }

    /// Find a class based on the name
    pub fn find_class(&self, fqn: &str) -> Option<ClassStatement> {
        return self.classes.get(fqn).cloned();
    }

    /// Check if the opening tag is on the right position.
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

    /// Analase the code.
    pub fn analyze(&mut self, file: &mut File) -> &mut Project {
        let statement = file.ast.clone();
        let mut project = self;
        match statement {
            Statement::FullOpeningTag(tag) => project = project.opening_tag(tag.span, file),
            Statement::ShortOpeningTag(tag) => {
                project = project.opening_tag(tag.span, file);
            }
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
                project.class_statement_analyze(ClassStatement, file);
            }
            Statement::Trait(_TraitStatement) => {}
            Statement::Interface(_InterfaceStatement) => {}
            Statement::If(_IfStatement) => {}
            Statement::Switch(_SwitchStatement) => {}
            Statement::Echo(_EchoStatement) => {}
            Statement::Expression(ExpressionStatement) => {
                project = project.analyze_expression(ExpressionStatement.expression, file)
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

    /// Analyse class statement.
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

    /// Analyse class member.
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
                let has_return = analyse::method_has_return(concretemethod.body.clone());

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

                let score = analyse::calculate_cyclomatic_complexity(concretemethod.body);
                // for statement in concretemethod.body.statements {
                //     // self.analyze_expression(statement, file);

                //     // println!("{statement:#?}");
                // }

                self
            }
            ClassMember::VariableProperty(variableproperty) => self,
            ClassMember::AbstractConstructor(_constructor) => self,
            ClassMember::ConcreteConstructor(constructor) => {
                for statement in constructor.body.statements {
                    match statement {
                        Statement::Expression(ExpressionStatement { expression, ending }) => {
                            self.analyze_expression(expression, file);
                        }

                        _ => {}
                    }
                }
                self
            }
        }
    }

    ///
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
    /// Analyze expressions.
    pub fn analyze_expression(&mut self, expresion: Expression, file: &mut File) -> &mut Project {
        let mut project = self;
        // println!("{expresion:#?}");
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
            Expression::MagicConstant(constant) => {}
            Expression::Bool(_) => {}
            Expression::AnonymousClass(class) => {}
            Expression::Nowdoc(_) => {}
            Expression::Heredoc(_) => {}
            Expression::ArrowFunction(function) => {}
            Expression::Closure(closure) => {}
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
                                    let exists =
                                        analyse::propperty_exists(identifier.clone(), file.clone());
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
            Expression::Instanceof(_) => {}
            Expression::Concat(_) => {}
            Expression::ArithmeticOperation(operation) => {}
            Expression::Literal(literal) => {}
            Expression::Print(_) => {}
            Expression::Unset(_) => {}
            Expression::Isset(_) => {}
            Expression::Empty(_) => {}
            Expression::AssignmentOperation(assignment) => match assignment {
                BitwiseOr {
                    left,
                    pipe_equals,
                    right,
                } => {}
                BitwiseAnd {
                    left,
                    ampersand_equals,
                    right,
                } => {}
                BitwiseOr {
                    left,
                    pipe_equals,
                    right,
                } => {}
                BitwiseXor {
                    left,
                    caret_equals,
                    right,
                } => {}
                LeftShift {
                    left,
                    left_shift_equals,
                    right,
                } => {}
                RightShift {
                    left,
                    right_shift_equals,
                    right,
                } => {}
                Coalesce {
                    left,
                    coalesce_equals,
                    right,
                } => {}
                Assign {
                    left,
                    equals,
                    right,
                } => {
                    project = project.analyze_expression(*left, file);
                }
                Addition {
                    left,
                    plus_equals,
                    right,
                } => {}
                Subtraction {
                    left,
                    minus_equals,
                    right,
                } => {}
                Multiplication {
                    left,
                    asterisk_equals,
                    right,
                } => {}
                Division {
                    left,
                    slash_equals,
                    right,
                } => {}
                Modulo {
                    left,
                    percent_equals,
                    right,
                } => {}
                Exponentiation {
                    left,
                    pow_equals,
                    right,
                } => {}
                Concat {
                    left,
                    dot_equals,
                    right,
                } => {}
            },
            Expression::Eval(_) => {}
            Expression::Die(_) => {}
            Expression::Noop {} => {}
            Expression::New(_) => {}
            Expression::Exit(_) => {}
            Expression::StaticVariableMethodClosureCreation(_) => {}
            Expression::StaticVariableMethodCall(_) => {}
            Expression::MethodClosureCreation(_) => {}
            Expression::AssignmentOperation(asignment) => {}
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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub path: PathBuf,

    pub ast: Statement,

    #[serde(skip_serializing, skip_deserializing)]
    pub members: Vec<ClassMember>,

    #[serde(skip_serializing, skip_deserializing)]
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug)]
pub enum Output {
    STDOUT,
    FILE,
}

impl File {
    fn get_line(&mut self, span: Span) -> String {
        println!("{}", self.path.display());
        return "".to_string();
    }
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
    ///
    pub fn start(&mut self) -> Option<php_parser_rs::parser::ast::classes::ClassStatement> {
        return match (self.ast.to_owned()) {
            Statement::Class(c) => Some(c),
            _ => None,
        };
    }
}
/// Parse the code and generate an ast.
pub fn parse_code(code: &str) -> Option<Vec<php_parser_rs::parser::ast::Statement>> {
    match parser::parse(code) {
        Ok(a) => Some(a),
        Err(r) => Some(vec![]),
    }
}
