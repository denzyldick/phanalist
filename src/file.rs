use std::path::PathBuf;

use php_parser_rs::parser;
use php_parser_rs::parser::ast::classes::ClassStatement;
use php_parser_rs::parser::ast::Statement;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub namespace: Option<String>,
    pub class_name: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub ast: Vec<Statement>,
}

impl File {
    pub fn new(path: PathBuf, content: String) -> Self {
        let ast = match parser::parse(&content) {
            Ok(a) => a,
            Err(_) => vec![],
        };

        Self {
            path: path.clone(),
            lines: content.lines().map(|s| s.to_string()).collect(),
            namespace: Self::get_namespace(&ast),
            class_name: Self::get_class_name(&ast),
            ast,
        }
    }

    fn get_namespace(ast: &[Statement]) -> Option<String> {
        let mut namespace: Option<String> = None;
        ast.iter().for_each(|statement| {
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

    fn get_class_name(ast: &[Statement]) -> Option<String> {
        let mut class_name: Option<String> = None;
        for statement in ast {
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
        match &self.namespace {
            Some(n) => {
                let option = self.class_name.clone();
                option.map(|s| format!("{}\\{}", n, s))
            }
            None => self.class_name.clone(),
        }
    }
}
