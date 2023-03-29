use colored::*;
use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::classes::{ClassExtends, ClassMember, ClassStatement};
use php_parser_rs::parser::ast::functions::{ConcreteMethod, FunctionParameterList, MethodBody};
use rocksdb::{IteratorMode, DB};
use std::sync::mpsc::Sender;

use jwalk::WalkDir;
use php_parser_rs::parser;
use php_parser_rs::parser::ast::identifiers::Identifier;
use php_parser_rs::parser::ast::modifiers::MethodModifierGroup;
use php_parser_rs::parser::ast::operators::AssignmentOperationExpression::*;
use php_parser_rs::parser::ast::properties::{Property, PropertyEntry};
use php_parser_rs::parser::ast::variables::Variable;
use php_parser_rs::parser::ast::ReturnStatement;
use php_parser_rs::parser::ast::Statement;
use php_parser_rs::parser::ast::{Expression, ExpressionStatement};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::analyse;

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
                            Err(_) => {
                                // println!("{err:?}");
                            }
                            Ok(content) => {
                                sender.send((content, path)).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Project {
    /// Iterate over the list of files and analyse the code.
    pub fn run(&mut self, db: &DB) {
        let iter = db.iterator(IteratorMode::Start);
        for i in iter {
            let item = i.unwrap();
            let file = item.1;

            match serde_json::from_slice::<File>(&file) {
                Err(e) => {
                    println!("{e}");
                }
                Ok(mut f) => {
                    f.ast = parse_code(f.content.as_str()).unwrap();
                    self.analyze(&mut f);
                }
            };
        }
    }

    /// Find a class based on the name
    pub fn find_class(&self, fqn: &str) -> Option<ClassStatement> {
        //todo find the class here.
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
        let mut project = self;

        for statement in file.ast.clone() {
            match statement {
                Statement::FullOpeningTag(tag) => project = project.opening_tag(tag.span, file),
                Statement::ShortOpeningTag(tag) => {
                    project = project.opening_tag(tag.span, file);
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
                    project = project.class_statement_analyze(class_statement, file);
                }
                Statement::Trait(_) => {}
                Statement::Interface(_) => {}
                Statement::If(_) => {}
                Statement::Switch(_) => {}
                Statement::Echo(_) => {}
                Statement::Expression(expression_statement) => {
                    project = project.analyze_expression(expression_statement.expression, file)
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
                                    project.class_statement_analyze(class_statement, file);
                                }
                                _ => {}
                            }
                        }
                    }
                    parser::ast::namespaces::NamespaceStatement::Braced(braced) => {
                        for statement in braced.body.statements {
                            match statement {
                                Statement::Class(class_statement) => {
                                    project.class_statement_analyze(class_statement, file);
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
            }
        }
        file.output(Output::STDOUT);
        project
    }

    /// Analyse class statement.
    pub fn class_statement_analyze(
        &mut self,
        class_statement: ClassStatement,
        file: &mut File,
    ) -> &mut Project {
        let mut project = self;
        let name = String::from(class_statement.name.value);
        match analyse::has_capitalized_name(name.clone(), class_statement.class) {
            Some(s) => {
                file.suggestions.push(s);
            }
            None => {}
        }

        for member in class_statement.body.members {
            file.members.push(member.clone());
            project = project.class_member_analyze(member, file);
        }

        let extends = class_statement.extends;
        match extends {
            Some(ClassExtends { extends, parent }) => {
                let exists = project.find_class(String::from(parent.value.clone()).as_str());
                match exists {
                    None => file.suggestions.push(Suggestion::from(
                        format!(
                            "{} is extending a class({}) that doesnt exits.",
                            name, parent.value
                        ),
                        class_statement.class,
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
                    if analyse::uppercased_constant_name(entry.clone()) == false {
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
            ClassMember::AbstractMethod(_) => self,
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
                match concretemethod.body.clone() {
                    MethodBody {
                        comments,
                        left_brace,
                        statements,
                        right_brace,
                    } => {
                        for statement in statements {
                            match statement {
                                Statement::Expression(ExpressionStatement {
                                    expression,
                                    ending,
                                }) => {
                                    self.analyze_expression(expression, file);
                                }

                                Statement::Expression(ExpressionStatement {
                                    expression,
                                    ending,
                                }) => match expression {
                                    Expression::MethodCall(method_call_expression) => {
                                        match *method_call_expression.method {
                                            _ => {}
                                            Expression::Identifier(
                                                Identifier::SimpleIdentifier(s),
                                            ) => {
                                                match *method_call_expression.target {
                                                    Expression::Variable(
                                                        Variable::SimpleVariable(s),
                                                    ) => {
                                                        if s.name.to_string()
                                                            == String::from("$this")
                                                        {
                                                            let mut exists = false;
                                                            for member in file.members.iter() {
                                                                match member.clone() {
                                                                    ClassMember::ConcreteMethod(
                                                                        ConcreteMethod {
                                                                            comments,
                                                                            attributes,
                                                                            modifiers,
                                                                            function,
                                                                            ampersand,
                                                                            name,
                                                                            parameters,
                                                                            return_type,
                                                                            body,
                                                                        },
                                                                    ) => {
                                                                        if exists == false
                                                                            && name.to_string()
                                                                                == String::from(
                                                                                    s.name.clone(),
                                                                                )
                                                                        {
                                                                            exists = true;
                                                                        }
                                                                    }
                                                                    _ => {}
                                                                };
                                                            }
                                                            if exists == false {
                                                                let suggestion = Suggestion::from(
                                                                                    format!(
                                                                                        "The method {} is being called but it doesn't exists. ",
                                                                                        String::from(s.name)
                                                                                        ),
                                                                                    s.span);
                                                                file.suggestions.push(suggestion);
                                                            };
                                                        };
                                                    }
                                                    _ => {}
                                                };
                                            }
                                        };
                                    }
                                    _ => {}
                                },
                                _ => {}
                            };
                        }
                    }
                };

                // Detect return statement without the proper return type signature.
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

                self
            }
            ClassMember::VariableProperty(_) => self,
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
                                    let exists =
                                        analyse::propperty_exists(identifier.clone(), file.clone());
                                    let name = analyse::get_property_name(identifier.clone());
                                    let span: Span = match identifier {
                    php_parser_rs::parser::ast::identifiers::Identifier::SimpleIdentifier(
                        identifier,
                    ) => Some(identifier.span),
                    _ => Some(Span { line: 0, column: 0, position: 0 })   // todo fix this
                                            ,
                }.unwrap();
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
            Expression::Variable(_) => {}
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
                    project = project.analyze_expression(*left, file);
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

    pub content: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub ast: Vec<Statement>,

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
                    for suggestion in &self.suggestions {
                        println!("\t{}", suggestion.suggestion.bold());
                        for (i, line) in self.content.lines().enumerate() {
                            if i == suggestion.span.line - 1 {
                                println!(
                                    "  {}\t{} {}",
                                    format!("{}:{}", suggestion.span.line, suggestion.span.column)
                                        .bold(),
                                    line_symbol,
                                    line.bold()
                                );
                            }
                        }
                        println!("");
                    }
                    println!("")
                }
            }
            Output::FILE => {}
        }
    }
}
/// Parse the code and generate an ast.
pub fn parse_code(code: &str) -> Option<Vec<php_parser_rs::parser::ast::Statement>> {
    match parser::parse(code) {
        Ok(a) => Some(a),
        Err(_) => Some(vec![]),
    }
}
