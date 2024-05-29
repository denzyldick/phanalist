use std::{collections::HashMap, path::PathBuf};

use php_parser_rs::parser::ast::classes::{ClassMember, ClassStatement};
use php_parser_rs::parser::ast::functions::MethodBody;
use php_parser_rs::parser::ast::identifiers::Identifier;
use php_parser_rs::parser::ast::modifiers::MethodModifierGroup;
use php_parser_rs::parser::ast::namespaces::{
    BracedNamespace, NamespaceStatement, UnbracedNamespace,
};
use php_parser_rs::parser::ast::{ExpressionStatement, MethodCallExpression, Statement};
use php_parser_rs::{lexer::byte_string::ByteString, parser};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub namespace: Option<String>,
    pub class_name: Option<String>,
    pub reference_counter: RC,
    #[serde(skip_serializing, skip_deserializing)]
    pub ast: Vec<Statement>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RC {
    methods: HashMap<ByteString, isize>,
}

impl RC {
    ///
    /// Increase the refrence of the the method.
    pub fn add_reference(&mut self, name: ByteString) {
        let value = self.methods.get(&name);

        if let Some(size) = value {
            let increased = size.clone() + 1;

            self.methods.insert(name.clone(), increased);
        }

        self.methods.insert(name, 1);
    }
    /// The idea is to traverse the ast an build a reference counter for the methods.
    ///
    pub fn build_reference_counter(&mut self, ast: &[Statement]) -> Option<RC> {
        for statement in ast.iter() {
            if let Statement::Class(class) = statement {
                for member in &class.body.members {
                    if let ClassMember::ConcreteMethod(method) = member {
                        self.add_reference(method.name.value.clone());
                        let mut r = false;
                        match &method.modifiers {
                            MethodModifierGroup { modifiers } => for modifier in modifiers {},
                            _ => (),
                        }
                        if r {
                            let MethodBody { statements, .. } = &method.body;
                            self.build_reference_counter(statements);
                        }
                    }
                    if let ClassMember::ConcreteConstructor(constructor) = member {
                        let MethodBody {
                            statements,
                            comments: _,
                            left_brace: _,
                            right_brace: _,
                        }: &MethodBody = &constructor.body;

                        let _exists = statements.iter().filter(|statements| true);
                    }
                    if let Statement::Expression(ExpressionStatement {
                        expression,
                        ending: _,
                    }) = statement
                    {
                        if let php_parser_rs::parser::ast::Expression::MethodCall(call) =
                            &expression
                        {
                            match *call.method.clone() {
                                parser::ast::Expression::MethodCall(MethodCallExpression {
                                    target,
                                    arrow,
                                    method,
                                    arguments,
                                }) => {
                                    dbg!(*method);
                                }

                                parser::ast::Expression::StaticMethodCall(_) => todo!(),

                                parser::ast::Expression::StaticVariableMethodCall(_) => {
                                    todo!()
                                }
                                _ => {}
                            };
                        }
                    }
                }
            }
        }
        None
    }

    // get the current amount
    pub fn get_counter(&mut self, name: ByteString) -> isize {
        let counter = self.methods.get(&name);

        if let Some(value) = counter {
            return *value;
        }

        self.methods.insert(name, 0);
        return 0;
    }

    fn new() -> Self {
        Self {
            methods: HashMap::new(),
        }
    }
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
            reference_counter: RC::new(),
            ast,
        }
    }
    /// .
    pub fn get_class(self, namespace: NamespaceStatement) -> Vec<Statement> {
        if let NamespaceStatement::Braced(BracedNamespace {
            namespace,
            name,
            body,
        }) = &namespace
        {
            return body.statements.clone();
        }
        if let NamespaceStatement::Unbraced(UnbracedNamespace {
            start,
            name,
            end,
            statements,
        }) = &namespace
        {
            return statements.clone();
        } else {
            return vec![];
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
                    Statement::Namespace(NamespaceStatement::Braced(n)) => {
                        for statement in &n.body.statements {
                            if let Statement::Class(ClassStatement { name, .. }) = statement {
                                class_name = Some(name.value.to_string());
                            }
                        }
                    }
                    Statement::Namespace(NamespaceStatement::Unbraced(n)) => {
                        for statement in &n.statements {
                            if let Statement::Class(ClassStatement { name, .. }) = statement {
                                class_name = Some(name.value.to_string());
                            }
                        }
                    }
                    _ => {}
                };
                if let Statement::Class(ClassStatement { name, .. }) = statement {
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
