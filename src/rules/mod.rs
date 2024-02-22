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
pub mod e11;
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

    #[allow(clippy::only_used_in_recursion)]
    #[allow(clippy::borrowed_box)]
    fn flatten_statements<'a>(&'a self, statement: &'a Statement) -> Vec<&Statement> {
        let mut expanded_statements: Vec<&Statement> = Vec::new();
        expanded_statements.push(statement);

        match statement {
            Statement::Try(s) => {
                for catch in &s.catches {
                    let CatchBlock {
                        start: _,
                        end: _,
                        types: _,
                        var: _,
                        body,
                    } = catch;
                    for statement in body {
                        expanded_statements.append(&mut self.flatten_statements(statement));
                    }
                }
            }
            Statement::Class(ClassStatement {
                attributes: _,
                modifiers: _,
                class: _,
                name: _,
                extends: _,
                implements: _,
                body,
            }) => {
                for member in &body.members {
                    match member {
                        php_parser_rs::parser::ast::classes::ClassMember::ConcreteMethod(
                            concrete_method,
                        ) => {
                            let statements = &concrete_method.body.statements;

                            for statement in statements {
                                expanded_statements.append(&mut self.flatten_statements(statement));
                            }
                        }
                        php_parser_rs::parser::ast::classes::ClassMember::ConcreteConstructor(
                            concrete_constructor,
                        ) => {
                            let statements = &concrete_constructor.body.statements;

                            for statement in statements {
                                expanded_statements.append(&mut self.flatten_statements(statement));
                            }
                        }
                        _ => {}
                    };
                }
            }
            Statement::If(if_statement) => {
                let IfStatement {
                    r#if: _,
                    left_parenthesis: _,
                    condition: _,
                    right_parenthesis: _,
                    body,
                } = if_statement;
                {
                    match body {
                        IfStatementBody::Block {
                            colon: _,
                            statements,
                            elseifs: _,
                            r#else: _,
                            endif: _,
                            ending: _,
                        } => {
                            for statement in statements {
                                expanded_statements.append(&mut self.flatten_statements(statement));
                            }
                        }
                        IfStatementBody::Statement {
                            statement,
                            elseifs: _,
                            r#else: _,
                        } => expanded_statements.append(&mut self.flatten_statements(statement)),
                    };
                }
            }
            Statement::While(while_statement) => match &while_statement.body {
                WhileStatementBody::Block {
                    colon: _,
                    statements,
                    endwhile: _,
                    ending: _,
                } => {
                    for statement in statements {
                        expanded_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                WhileStatementBody::Statement { statement } => {
                    expanded_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::Switch(SwitchStatement {
                switch: _,
                left_parenthesis: _,
                condition: _,
                right_parenthesis: _,
                cases,
            }) => {
                for case in cases {
                    for statement in &case.body {
                        expanded_statements.append(&mut self.flatten_statements(statement))
                    }
                }
            }
            Statement::Foreach(ForeachStatement {
                foreach: _,
                left_parenthesis: _,
                iterator: _,
                right_parenthesis: _,
                body,
            }) => match body {
                ForeachStatementBody::Block {
                    colon: _,
                    statements,
                    endforeach: _,
                    ending: _,
                } => {
                    for statement in statements {
                        expanded_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                ForeachStatementBody::Statement { statement } => {
                    expanded_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::For(for_statement_body) => match &for_statement_body.body {
                ForStatementBody::Block {
                    colon: _,
                    statements,
                    endfor: _,
                    ending: _,
                } => {
                    for statement in statements {
                        expanded_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                ForStatementBody::Statement { statement } => {
                    expanded_statements.append(&mut self.flatten_statements(statement));
                }
            },
            Statement::Block(BlockStatement {
                left_brace: _,
                statements,
                right_brace: _,
            }) => {
                for statement in statements {
                    expanded_statements.append(&mut self.flatten_statements(statement));
                }
            }

            Statement::Namespace(namespace) => match &namespace {
                namespaces::NamespaceStatement::Unbraced(unbraced) => {
                    for statement in &unbraced.statements {
                        expanded_statements.append(&mut self.flatten_statements(statement));
                    }
                }
                namespaces::NamespaceStatement::Braced(braced) => {
                    for statement in &braced.body.statements {
                        expanded_statements.append(&mut self.flatten_statements(statement));
                    }
                }
            },

            _ => {}
        };

        expanded_statements
    }
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
    add_rule(&mut rules, Box::default() as Box<e11::Rule>);

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
}
