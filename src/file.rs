use std::{collections::HashMap, path::PathBuf};

use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::classes::{ClassMember, ClassStatement};
use php_parser_rs::parser::ast::functions::MethodBody;
use php_parser_rs::parser::ast::identifiers::{Identifier, SimpleIdentifier};
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
    pub methods: HashMap<ByteString, Method>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Method {
    pub name: ByteString,
    pub span: Span,
    pub counter: isize,
}

impl Method {
    pub fn increase_counter(&mut self) {
        self.counter += 1;
    }
}
impl RC {
    ///
    /// Increase the refrence of the the method.
    pub fn add_reference(&mut self, identifier: SimpleIdentifier) {
        let value = self.methods.get(&identifier.value);

        if let Some(method) = value {
            let mut m = method.clone();
            m.increase_counter();
            self.methods.insert(identifier.value, m);
        } else {
            let method = Method {
                name: identifier.value.clone(),
                span: identifier.span,
                counter: 0_isize,
            };
            self.methods.insert(identifier.value, method);
        }
    }

    /// The idea is to traverse the ast an build a reference counter for the methods.
    ///
    pub fn build_reference_counter(&mut self, ast: &[Statement]) -> Option<RC> {
        for statement in ast.iter() {
            if let Statement::Class(class) = statement {
                for member in &class.body.members {
                    if let ClassMember::ConcreteMethod(method) = member {
                        let mut is_private = false;
                        let MethodModifierGroup { modifiers } = &method.modifiers;
                        for modifier in modifiers {
                            if let parser::ast::modifiers::MethodModifier::Private(_m) = modifier {
                                is_private = true;
                                dbg!(is_private);
                                self.add_reference(method.name.clone());
                            }
                        }
                        if is_private {
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

                        let _exists = statements.iter().filter(|_statements| true);
                    }
                }
            }
            if let Statement::Expression(ExpressionStatement {
                expression,
                ending: _,
            }) = statement
            {
                let name = match expression {
                    parser::ast::Expression::MethodCall(MethodCallExpression {
                        target: _,
                        arrow: _,
                        method,
                        arguments: _,
                    }) => match *method.clone() {
                        parser::ast::Expression::Identifier(Identifier::SimpleIdentifier(s)) => {
                            Some(s)
                        }
                        _ => None,
                    },
                    parser::ast::Expression::MethodClosureCreation(_) => None,
                    parser::ast::Expression::NullsafeMethodCall(_) => None,
                    parser::ast::Expression::StaticMethodCall(_) => None,
                    parser::ast::Expression::StaticVariableMethodCall(_) => None,
                    parser::ast::Expression::StaticMethodClosureCreation(_) => None,
                    parser::ast::Expression::StaticVariableMethodClosureCreation(_) => None,
                    _ => None,
                };

                if let Some(name) = name {
                    self.add_reference(name);
                }
            }
        }

        None
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
    ///
    /// Return the statements in a class.
    pub fn get_class(&self) -> Option<Vec<Statement>> {
        let namespace = self
            .ast
            .iter()
            .filter(|statement| {
                if let Statement::Namespace(_) = statement {
                    return true;
                }
                false
            })
            .next();

        if let Some(Statement::Namespace(n)) = namespace {
            return match n {
                NamespaceStatement::Unbraced(UnbracedNamespace {
                    start: _,
                    name: _,
                    end: _,
                    statements,
                }) => Some(statements.clone()),
                NamespaceStatement::Braced(BracedNamespace {
                    namespace: _,
                    name: _,
                    body,
                }) => Some(body.statements.clone()),
            };
        }
        None
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
