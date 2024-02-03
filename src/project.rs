use std::collections::HashMap;
use std::default::Default;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use colored::*;
use jwalk::WalkDir;
use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser;
use php_parser_rs::parser::ast::classes::{ClassMember, ClassStatement};
use php_parser_rs::parser::ast::functions::ConcreteMethod;
use php_parser_rs::parser::ast::namespaces::{NamespaceStatement, UnbracedNamespace};
use php_parser_rs::parser::ast::Statement;
use rocksdb::{IteratorMode, DB};
use serde::{Deserialize, Serialize};

use crate::analyse::Analyse;
use crate::config::{Config, Output};
use crate::storage;

pub struct Project {
    pub files: Vec<File>,
    pub classes: HashMap<String, ClassStatement>,
    pub config: Config,
    db: Option<DB>,
    analyse: Option<Analyse>,
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
        if (file_name != "." || !file_name.is_empty()) && metadata.is_file() {
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

impl Project {
    pub fn new(config_path: PathBuf, src: Option<String>) -> Self {
        let mut project = Self {
            files: Vec::new(),
            classes: HashMap::new(),
            config: Config::default(),
            db: None,
            analyse: None,
        };
        project.parse_config(config_path);

        if let Some(src) = src {
            project.config.src = src;
        }

        let file_path = project.config.storage.clone();
        let file = std::path::Path::new(&file_path);

        if file.is_dir() {
            let _ = fs::remove_dir_all(file);
        }
        project.db = Some(DB::open_default(&project.config.storage).unwrap());

        project.analyse = Some(Analyse::new(project.config.clone()));

        project
    }
    /// Iterate over the list of files and analyse the code.
    pub fn run(&mut self) {
        let db = self.db.as_mut().unwrap();
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
                    Self::analyze(f, self.analyse.as_ref().unwrap());
                }
            };
        }
    }

    pub fn scan(&self) -> i64 {
        let (send, recv) = std::sync::mpsc::channel();
        let path = self.config.src.clone();
        std::thread::spawn(move || {
            let path = PathBuf::from(path);
            self::scan_folder(path, send);
        });
        let file_path = self.config.storage.clone();
        let file = std::path::Path::new(&file_path);

        if file.is_dir() {
            let _ = fs::remove_dir_all(file);
        }

        let db = self.db.as_ref().unwrap();
        let mut files = 0;
        for (content, path) in recv {
            let ast = parse_code(&content).unwrap();
            let file = &mut File {
                content,
                path: path.clone(),
                ast: ast.clone(),
                members: Vec::new(),
                methods: Vec::new(),
                suggestions: Vec::new(),
            };

            file.build_metadata();
            if let Some(fqn) = file.get_fully_qualified_name() {
                storage::put(db, fqn, file.clone());
                files += 1;
            };
        }
        files
    }

    /// Parse the configuration yaml file.
    /// If there is no configuration file, create a new new one.
    pub fn parse_config(&mut self, path: PathBuf) {
        match fs::read_to_string(path.clone()) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                println!(
                    "No configuration file {} has been found.",
                    &path.clone().display()
                );
                println!("Do you want to create a configuration file (otherwise defaults will be used)? [Y/n]");

                let mut answer = String::new();
                std::io::stdin().read_line(&mut answer).unwrap();

                if answer.trim().to_lowercase() == "y" || answer.trim().to_lowercase() == "yes" {
                    self.config.save(path.clone());
                    println!(
                        "The new {} configuration file as been created",
                        &path.display()
                    );
                };
            }

            Err(e) => {
                panic!("{}", e)
            }
            Ok(s) => {
                println!("Using configuration file {}", &path.display());
                match serde_yaml::from_str(&s) {
                    Ok(c) => {
                        self.config = c;
                    }
                    Err(e) => {
                        println!("Unable to use the config: {}. Ignoring it.", &e);
                    }
                }
            }
        };
    }

    pub fn analyze(mut file: File, analyse: &Analyse) -> Vec<Suggestion> {
        for statement in file.ast.clone() {
            let suggestions = analyse.statement(statement);
            for suggestion in suggestions {
                file.suggestions.push(suggestion);
            }
        }
        file.output(Output::STDOUT);
        file.suggestions
    }
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    rule: String,
    suggestion: String,
    span: Span,
}

impl Suggestion {
    pub fn from(suggesion: String, span: Span, rule: String) -> Self {
        Self {
            rule,
            suggestion: suggesion,
            span,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub path: PathBuf,

    pub content: String,

    #[serde(skip_serializing, skip_deserializing)]
    pub ast: Vec<Statement>,

    pub members: Vec<ClassMember>,

    #[serde(skip_serializing, skip_deserializing)]
    pub suggestions: Vec<Suggestion>,

    pub methods: Vec<ConcreteMethod>,
}

impl File {
    /// Build metadata public methods, variables, contstants, etc.
    /// @todo add properties and constants.
    pub fn build_metadata(&mut self) {
        self.ast.iter().for_each(|statement| {
            if let Statement::Namespace(NamespaceStatement::Unbraced(UnbracedNamespace {
                start: _,
                name: _,
                end: _,
                statements,
            })) = statement
            {
                statements.iter().for_each(|statement| {
                    if let Statement::Class(ClassStatement {
                        attributes: _,
                        modifiers: _,
                        class: _,
                        name: _,
                        extends: _,
                        implements: _,
                        body,
                    }) = statement
                    {
                        for member in &body.members {
                            if let php_parser_rs::parser::ast::classes::ClassMember::ConcreteMethod(
                                _concrete_method,
                            ) = member {
                                self.members.push(member.clone());
                            };
                        }
                    };
                })
            };

            if let Statement::Class(ClassStatement {
                attributes: _,
                modifiers: _,
                class: _,
                name: _,
                extends: _,
                implements: _,
                body,
            }) = statement
            {
                for member in &body.members {
                    if let php_parser_rs::parser::ast::classes::ClassMember::ConcreteMethod(
                        _concrete_method,
                    ) = member
                    {
                        self.members.push(member.clone());
                    };
                }
            };
        });
    }

    /// Return the namespace of the statement.
    fn get_namespace(&self) -> Option<String> {
        let mut namespace: Option<String> = None;
        self.ast.iter().for_each(|statement| {
            namespace = match statement {
                Statement::Namespace(parser::ast::namespaces::NamespaceStatement::Braced(n)) => {
                    if n.name.is_some() {
                        Some(n.name.clone().unwrap().value.to_string())
                    } else {
                        None
                    }
                }
                Statement::Namespace(parser::ast::namespaces::NamespaceStatement::Unbraced(n)) => {
                    Some(n.name.to_string())
                }
                _ => None,
            };
        });
        namespace
    }

    /// Get the class name inside a method body
    fn get_class_name(&self) -> Option<String> {
        let mut class_name: Option<String> = None;
        for statement in &self.ast {
            if class_name.is_none() {
                match statement {
                    Statement::Namespace(parser::ast::namespaces::NamespaceStatement::Braced(
                        n,
                    )) => {
                        for statement in &n.body.statements {
                            if let Statement::Class(ClassStatement {
                                attributes: _,
                                modifiers: _,
                                class: _,
                                name,
                                extends: _,
                                implements: _,
                                body: _,
                            }) = statement
                            {
                                class_name = Some(name.value.to_string());
                            }
                        }
                    }
                    Statement::Namespace(
                        parser::ast::namespaces::NamespaceStatement::Unbraced(n),
                    ) => {
                        for statement in &n.statements {
                            if let Statement::Class(ClassStatement {
                                attributes: _,
                                modifiers: _,
                                class: _,
                                name,
                                extends: _,
                                implements: _,
                                body: _,
                            }) = statement
                            {
                                class_name = Some(name.value.to_string());
                            }
                        }
                    }
                    _ => {}
                };
                if let Statement::Class(ClassStatement {
                    attributes: _,
                    modifiers: _,
                    class: _,
                    name,
                    extends: _,
                    implements: _,
                    body: _,
                }) = statement
                {
                    class_name = Some(name.value.to_string());
                }
            }
        }
        class_name
    }

    pub fn get_fully_qualified_name(&self) -> Option<String> {
        match self.get_namespace() {
            Some(n) => {
                let option = self.get_class_name();
                option.map(|s| format!("{}\\{}", n, s))
            }
            None => self.get_class_name(),
        }
    }

    pub fn output(&mut self, location: Output) {
        match location {
            Output::STDOUT => {
                if !self.suggestions.is_empty() {
                    let file_symbol = "--->".blue().bold();
                    println!("{} {} ", file_symbol, self.path.display());
                    println!(
                        "{} {}",
                        "Warnings detected: ".yellow().bold(),
                        self.suggestions.len().to_string().as_str().red().bold()
                    );
                    let line_symbol = "|".blue().bold();
                    for suggestion in &self.suggestions {
                        println!(
                            "  {}:\t{}",
                            suggestion.rule.yellow().bold(),
                            suggestion.suggestion.bold()
                        );
                        for (i, line) in self.content.lines().enumerate() {
                            if i == suggestion.span.line - 1 {
                                println!(
                                    "  {}\t{} {}",
                                    format!("{}:{}", suggestion.span.line, suggestion.span.column)
                                        .blue()
                                        .bold(),
                                    line_symbol,
                                    line
                                );
                            }
                        }
                        println!();
                    }
                    println!()
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
