use std::collections::HashMap;
use std::default::Default;

use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::classes::ClassStatement;
use php_parser_rs::parser::ast::control_flow::{IfStatement, IfStatementBody};
use php_parser_rs::parser::ast::loops::{
    ForStatementBody, ForeachStatement, ForeachStatementBody, WhileStatementBody,
};
use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::{namespaces, BlockStatement, Statement, SwitchStatement};
use serde_json::Value;

use crate::config::Config;
use crate::file::File;
use crate::results::Violation;

pub mod e0;
pub mod e1;
pub mod e10;
pub mod e12;
pub mod e2;
pub mod e3;
pub mod e4;
pub mod e5;
pub mod e6;
pub mod e7;
pub mod e8;
pub mod e9;

pub trait Rule {
    // Would be a good idea to have default implementation which extracts the code from struct name
    // Haven't found a way to implement it
    fn get_code(&self) -> String;

    fn description(&self) -> String {
        String::from("")
    }

    fn set_config(&mut self, _json: &Value) {}

    fn read_config(&mut self, config: &Config) {
        let code = self.get_code();
        if let Some(rule_config) = config.rules.get(&code) {
            self.set_config(rule_config);
        }
    }

    fn do_validate(&self, file: &File) -> bool {
        file.get_fully_qualified_name().is_some()
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation>;

    fn new_violation(&self, file: &File, suggestion: String, span: Span) -> Violation {
        let line = file.lines.get(span.line - 1).unwrap();

        Violation {
            rule: self.get_code(),
            line: String::from(line),
            suggestion,
            span,
        }
    }

    fn flatten_statements<'a>(&'a self, statement: &'a Statement) -> Vec<&Statement> {
        let mut flatten_statements: Vec<&Statement> = Vec::new();
        flatten_statements.push(statement);

        match statement {
            Statement::Try(try_statement) => {
                for catch in &try_statement.catches {
                    let CatchBlock { body, .. } = catch;
                    for statement in body {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
            }
            Statement::Class(ClassStatement { body, .. }) => {
                for member in &body.members {
                    match member {
                        php_parser_rs::parser::ast::classes::ClassMember::ConcreteMethod(
                            method,
                        ) => {
                            for statement in &method.body.statements {
                                flatten_statements.append(&mut self.flatten_statements(statement));
                            }
                        }
                        php_parser_rs::parser::ast::classes::ClassMember::ConcreteConstructor(
                            constructor,
                        ) => {
                            for statement in &constructor.body.statements {
                                flatten_statements.append(&mut self.flatten_statements(statement));
                            }
                        }
                        _ => {}
                    };
                }
            }
            Statement::If(if_statement) => {
                let IfStatement { body, .. } = if_statement;
                {
                    match body {
                        IfStatementBody::Block { statements, .. } => {
                            for statement in statements {
                                flatten_statements.append(&mut self.flatten_statements(statement));
                            }
                        }
                        IfStatementBody::Statement { statement, .. } => {
                            flatten_statements.append(&mut self.flatten_statements(statement))
                        }
                    };
                }
            }
            Statement::While(while_statement) => match &while_statement.body {
                WhileStatementBody::Block { statements, .. } => {
                    for statement in statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                WhileStatementBody::Statement { statement } => {
                    flatten_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::Switch(SwitchStatement { cases, .. }) => {
                for case in cases {
                    for statement in &case.body {
                        flatten_statements.append(&mut self.flatten_statements(statement))
                    }
                }
            }
            Statement::Foreach(ForeachStatement { body, .. }) => match body {
                ForeachStatementBody::Block { statements, .. } => {
                    for statement in statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                ForeachStatementBody::Statement { statement } => {
                    flatten_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::For(for_statement_body) => match &for_statement_body.body {
                ForStatementBody::Block { statements, .. } => {
                    for statement in statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                ForStatementBody::Statement { statement } => {
                    flatten_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::Block(BlockStatement { statements, .. }) => {
                for statement in statements {
                    flatten_statements.append(&mut self.flatten_statements(statement));
                }
            }
            Statement::Namespace(namespace) => match &namespace {
                namespaces::NamespaceStatement::Unbraced(unbraced) => {
                    for statement in &unbraced.statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                namespaces::NamespaceStatement::Braced(braced) => {
                    for statement in &braced.body.statements {
                        flatten_statements.append(&mut self.flatten_statements(statement));
                    }
                }
            },
            _ => {}
        };

        flatten_statements
    }
}

pub(crate) fn do_validate_namespace(
    ns: String,
    include: &Vec<String>,
    exclude: &Vec<String>,
) -> bool {
    for exclude_ns in exclude {
        if ns.contains(exclude_ns) {
            return false;
        }
    }

    if !include.is_empty() {
        for include_ns in include {
            if ns.contains(include_ns) {
                return true;
            }
        }

        return false;
    }

    true
}

fn add_rule(rules: &mut HashMap<String, Box<dyn Rule>>, rule: Box<dyn Rule>) {
    rules.insert(rule.get_code(), rule as Box<dyn Rule>);
}

pub fn all_rules() -> HashMap<String, Box<dyn Rule>> {
    let mut rules: HashMap<String, Box<dyn Rule>> = HashMap::new();

    add_rule(&mut rules, Box::new(e1::Rule {}));
    add_rule(&mut rules, Box::new(e2::Rule {}));
    add_rule(&mut rules, Box::new(e3::Rule {}));
    add_rule(&mut rules, Box::new(e4::Rule {}));
    add_rule(&mut rules, Box::new(e5::Rule {}));
    add_rule(&mut rules, Box::new(e6::Rule {}));
    add_rule(&mut rules, Box::default() as Box<e7::Rule>);
    add_rule(&mut rules, Box::new(e8::Rule {}));
    add_rule(&mut rules, Box::default() as Box<e9::Rule>);
    add_rule(&mut rules, Box::default() as Box<e10::Rule>);
    add_rule(&mut rules, Box::default() as Box<e12::Rule>);

    rules
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::analyse::Analyse;

    use super::*;

    pub(crate) fn analyze_file_for_rule(path: &str, rule_code: &str) -> Vec<Violation> {
        let path = PathBuf::from(format!("./src/rules/examples/{path}"));
        let content = fs::read_to_string(&path).unwrap();
        let file = File::new(path, content);

        let config = Config {
            enabled_rules: vec![rule_code.to_string()],
            ..Default::default()
        };
        let analyse = Analyse::new(&config);

        analyse.analyse_file(&file)
    }

    fn get_ns() -> String {
        "App\\Service\\Search".to_string()
    }

    #[test]
    fn do_validate_namespace_empty_include_and_exclude() {
        assert!(do_validate_namespace(get_ns(), &vec![], &vec![]));
    }

    #[test]
    fn do_validate_namespace_include_contains() {
        assert!(do_validate_namespace(
            get_ns(),
            &vec!["\\Service\\".to_string()],
            &vec![]
        ));
    }

    #[test]
    fn do_validate_namespace_include_not_contains() {
        assert!(!do_validate_namespace(
            get_ns(),
            &vec!["\\Service2\\".to_string()],
            &vec![]
        ));
    }

    #[test]
    fn do_validate_namespace_exclude_contains() {
        assert!(!do_validate_namespace(
            get_ns(),
            &vec![],
            &vec!["\\Service\\".to_string()],
        ));
    }

    #[test]
    fn do_validate_namespace_exclude_not_contains() {
        assert!(do_validate_namespace(
            get_ns(),
            &vec![],
            &vec!["\\Service2\\".to_string()],
        ));
    }

    #[test]
    fn do_validate_namespace_include_contains_exclude_contains() {
        let namespaces = &vec!["\\Service\\".to_string()];
        assert!(!do_validate_namespace(get_ns(), namespaces, namespaces));
    }
}
