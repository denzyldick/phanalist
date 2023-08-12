use colored::*;
use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::classes::{ClassMember, ClassStatement};

use php_parser_rs::parser::ast::functions::ConcreteMethod;
use rocksdb::{IteratorMode, DB};
use std::io::{ErrorKind, Write};
use std::sync::mpsc::Sender;

use crate::analyse::Analyse;
use crate::storage;
use jwalk::WalkDir;
use php_parser_rs::parser;
use php_parser_rs::parser::ast::Statement;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Project {
    pub files: Vec<File>,
    pub classes: HashMap<String, ClassStatement>,
    pub config: Config,
    pub working_dir: PathBuf,
    db: Option<DB>,
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

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Config {
    pub src: String,
    pub storage: String,
    disable: Vec<String>,
    output: Output,
}

impl Project {
    pub fn new(work_dir: PathBuf) -> Self {
        let mut project = Self {
            files: Vec::new(),
            classes: HashMap::new(),
            config: Config {
                src: String::new(),
                storage: String::new(),
                disable: Vec::new(),
                output: Output::STDOUT,
            },
            working_dir: work_dir,
            db: None,
        };
        project.parse_config();
        let file_path = project.config.storage.clone();
        let file = std::path::Path::new(&file_path);

        if file.is_dir() {
            match fs::remove_dir_all(file) {
                Ok(_) => {}
                Err(_) => {}
            }
        }
        project.db = Some(DB::open_default(&project.config.storage).unwrap());
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
                    Self::analyze(f, self.config.disable.clone());
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
            match fs::remove_dir_all(file) {
                Ok(_) => {}
                Err(_) => {}
            }
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
            storage::put(&db, file.get_fully_qualified_name().unwrap(), file.clone());
            files = files + 1;
        }
        files
    }

    /// Parse the configuration yaml file.
    /// If there is no configuration file, create a new new one.
    pub fn parse_config(&mut self) {
        let path = format!("{}/phanalist.yaml", self.working_dir.display());
        println!("{}", self.working_dir.display());
        println!("{}", path);
        self.config = match fs::read_to_string(path) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                println!("No configuration file named phanalist.yaml has been found. ");
                println!("Do you want to create configuration file? [Y/n]");

                let mut answer = String::new();

                std::io::stdin().read_line(&mut answer).unwrap();

                if answer.trim().to_lowercase() == "y" || answer.trim().to_lowercase() == "yes" {
                    let mut disable = Vec::new();
                    disable.push("DUMMY_ERROR".to_string());
                    let config = Config {
                        src: String::from("./"),
                        disable,
                        storage: String::from("/tmp/phanalist"),
                        output: Output::STDOUT,
                    };

                    let t = serde_yaml::to_string(&config).unwrap();
                    println!("The new configuration file as been created: phanalist.yaml");
                    println!("{t}");
                    let mut file = std::fs::File::create("./phanalist.yaml").unwrap();
                    file.write_all(t.as_bytes()).unwrap();
                    config
                } else {
                    Config {
                        src: String::from("./"),
                        disable: Vec::new(),
                        storage: String::from("/tmp/phanalist"),
                        output: Output::STDOUT,
                    }
                }
            }

            Err(e) => {
                panic!("{}", e)
            }
            Ok(s) => {
                println!("Reading configuration from phanalist.yml");
                serde_yaml::from_str(&s).unwrap()
            }
        };
    }

    /// Analyze the code.
    pub fn analyze(mut file: File, disable: Vec<String>) -> Vec<Suggestion> {
        let analyse: Analyse = Analyse::new(disable, file.clone());
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

    pub methods: Vec<ConcreteMethod>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Output {
    STDOUT,
    FILE,
}

impl File {
    /// Build metadata public methods, variables, contstants, etc.
    /// @todo add properties and constants.
    pub fn build_metadata(&mut self) {
        self.ast.iter().for_each(|statement| {
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
                    match member {
                        php_parser_rs::parser::ast::classes::ClassMember::ConcreteMethod(
                            _concrete_method,
                        ) => {
                            self.members.push(member.clone());
                        }
                        _ => {}
                    };
                }
            };
        });
    }
    /// Return the namespace of the statement.
    /// @todo make sure it also works with unbraced namepspace.
    fn get_namespace(&self) -> Option<String> {
        let mut namespace: Option<String> = None;
        self.ast.iter().for_each(|statement| {
            namespace = match statement {
                Statement::Namespace(parser::ast::namespaces::NamespaceStatement::Braced(n)) => {
                    Some(n.name.clone().unwrap().value.to_string())
                }
                Statement::Namespace(parser::ast::namespaces::NamespaceStatement::Unbraced(n)) => {
                    Some("".to_string())
                }
                _ => Some("".to_string()),
            };
        });
        namespace
    }

    /// Get the class name inside a method body
    /// @todo make sure it also works with unbraced namespace.
    fn get_class_name(&self) -> Option<String> {
        let mut class_name: Option<String> = None;
        for statement in &self.ast {
            match class_name {
                None => {
                    match statement {
                        Statement::Namespace(
                            parser::ast::namespaces::NamespaceStatement::Braced(n),
                        ) => {
                            for statement in &n.body.statements {
                                match statement {
                                    Statement::Class(ClassStatement {
                                        attributes: _,
                                        modifiers: _,
                                        class: _,
                                        name,
                                        extends: _,
                                        implements: _,
                                        body: _,
                                    }) => {
                                        class_name = Some(name.value.to_string());
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => {}
                    };
                    match statement {
                        Statement::Class(ClassStatement {
                            attributes: _,
                            modifiers: _,
                            class: _,
                            name,
                            extends: _,
                            implements: _,
                            body: _,
                        }) => {
                            class_name = Some(name.value.to_string());
                        }
                        _ => (),
                    }
                }
                _ => {}
            }
        }
        class_name
    }
    pub fn get_fully_qualified_name(&self) -> Option<String> {
        match self.get_namespace() {
            Some(n) => Some(format!("{}\\{}", n, self.get_class_name().unwrap())),
            None => Some(self.get_class_name().unwrap()),
        }
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
