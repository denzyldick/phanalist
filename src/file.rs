use std::{collections::HashMap, path::PathBuf};

use mago_ast::{Program, Statement};
use mago_interner::ThreadedInterner;
use mago_lexer::input::Input;
use mago_source::{SourceCategory, SourceIdentifier, SourceManager};
use mago_span::Span;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub namespace: Option<String>,
    pub class_name: Option<String>,
    pub reference_counter: RC,
    #[serde(skip_serializing, skip_deserializing)]
    pub ast: Option<Program>,
    #[serde(skip, default = "default_interner")]
    pub interner: ThreadedInterner,
    #[serde(skip, default = "default_source_manager")]
    pub source_manager: SourceManager,
    #[serde(skip, default = "default_source_id")]
    pub source_id: Option<SourceIdentifier>,
}

fn default_interner() -> ThreadedInterner {
    ThreadedInterner::new()
}

fn default_source_manager() -> SourceManager {
    SourceManager::new(ThreadedInterner::new())
}

fn default_source_id() -> Option<SourceIdentifier> {
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RC {
    pub methods: HashMap<String, Method>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Method {
    pub name: String,
    pub span: Span,
    pub counter: isize,
}

impl Method {
    pub fn increase_counter(&mut self) {
        self.counter += 1;
    }
}

impl RC {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
        }
    }

    // TODO: Implement referencing counting with Mago AST
    pub fn build_reference_counter(&mut self, _program: &Program) -> Option<RC> {
        None
    }
}

impl File {
    pub fn new(path: PathBuf, content: String) -> Self {
        let interner = ThreadedInterner::new();
        let source_manager = SourceManager::new(interner.clone());

        let source_id = source_manager.insert_content(
            path.to_string_lossy().to_string(),
            content.clone(),
            SourceCategory::UserDefined,
        );

        let source = source_manager.load(&source_id).unwrap();
        let content_str = interner.lookup(&source.content);
        let input = Input::new(source_id, content_str.as_bytes());

        // Parse
        let (program, _errors) = mago_parser::parse(&interner, input);

        let mut namespace = None;
        let mut class_name = None;

        for statement in program.statements.iter() {
            match statement {
                Statement::Namespace(ns) => {
                    if let Some(name) = &ns.name {
                        namespace = Some(interner.lookup(&name.value()).to_string());
                    }
                    for s in ns.statements().iter() {
                        if let Statement::Class(class) = s {
                            class_name = Some(interner.lookup(&class.name.value).to_string());
                            break;
                        }
                    }
                }
                Statement::Class(class) => {
                    class_name = Some(interner.lookup(&class.name.value).to_string());
                }
                _ => {}
            }

            if class_name.is_some() {
                break;
            }
        }

        Self {
            path: path.clone(),
            lines: content.lines().map(|s| s.to_string()).collect(),
            namespace,
            class_name,
            reference_counter: RC::new(),
            ast: Some(program),
            interner,
            source_manager,
            source_id: Some(source_id),
        }
    }

    pub fn get_class(&self) -> Option<Vec<mago_ast::Statement>> {
        // TODO: Implement with Mago AST
        None
    }

    pub fn get_fully_qualified_name(&self) -> Option<String> {
        match &self.namespace {
            Some(n) => {
                let option = self.class_name.clone();
                option.map(|s| format!("{}\\{}", n, s))
            }
            None => self.class_name.clone(),
        }
    }
}
