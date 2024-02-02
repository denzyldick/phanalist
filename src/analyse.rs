use std::collections::HashMap;

use php_parser_rs::parser;
use php_parser_rs::parser::ast::classes::ClassStatement;
use php_parser_rs::parser::ast::control_flow::{IfStatement, IfStatementBody};
use php_parser_rs::parser::ast::loops::{
    ForStatementBody, ForeachStatement, ForeachStatementBody, WhileStatementBody,
};
use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::{namespaces, BlockStatement, Statement, SwitchStatement};

use crate::config::Config;
use crate::project::Suggestion;
use crate::rules::Rule;
use crate::rules::{self};

pub struct Analyse {
    rules: HashMap<String, Box<dyn Rule>>,
}

impl Analyse {
    pub fn new(config: Config) -> Self {
        Self {
            rules: Self::get_active_rules(config),
        }
    }

    fn get_active_rules(config: Config) -> HashMap<String, Box<dyn Rule>> {
        let mut codes: Vec<String> = rules::all_rules().into_keys().collect();

        if !config.enabled_rules.is_empty() {
            codes.retain(|x| config.enabled_rules.contains(x));
        }

        if !config.disable_rules.is_empty() {
            codes.retain(|x| !config.disable_rules.contains(x));
        }

        let mut active_rules = rules::all_rules();
        active_rules.retain(|code, rule| {
            rule.read_config(config.clone());

            codes.contains(code)
        });
        active_rules
    }

    pub fn statement(&self, statement: parser::ast::Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        let rules = &self.rules;
        for (_, rule) in rules.iter() {
            suggestions.append(&mut self.expand(&statement, rule));
        }
        suggestions
    }

    #[allow(clippy::only_used_in_recursion)]
    #[allow(clippy::borrowed_box)]
    fn expand(&self, statement: &Statement, rule: &Box<dyn Rule>) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        suggestions.append(&mut rule.validate(statement));
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
                        suggestions.append(&mut self.expand(statement, rule));
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
                                suggestions.append(&mut self.expand(statement, rule));
                            }
                        }
                        php_parser_rs::parser::ast::classes::ClassMember::ConcreteConstructor(
                            concrete_constructor,
                        ) => {
                            let statements = &concrete_constructor.body.statements;

                            for statement in statements {
                                suggestions.append(&mut self.expand(statement, rule));
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
                                suggestions.append(&mut self.expand(statement, rule));
                            }
                        }
                        IfStatementBody::Statement {
                            statement,
                            elseifs: _,
                            r#else: _,
                        } => suggestions.append(&mut self.expand(statement, rule)),
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
                        suggestions.append(&mut self.expand(statement, rule));
                    }
                }
                WhileStatementBody::Statement { statement } => {
                    suggestions.append(&mut self.expand(statement, rule));
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
                        suggestions.append(&mut self.expand(statement, rule))
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
                        suggestions.append(&mut self.expand(statement, rule));
                    }
                }
                ForeachStatementBody::Statement { statement } => {
                    suggestions.append(&mut self.expand(statement, rule));
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
                        suggestions.append(&mut self.expand(statement, rule));
                    }
                }
                ForStatementBody::Statement { statement } => {
                    suggestions.append(&mut self.expand(statement, rule))
                }
            },
            Statement::Block(BlockStatement {
                left_brace: _,
                statements,
                right_brace: _,
            }) => {
                for statement in statements {
                    suggestions.append(&mut self.expand(statement, rule));
                }
            }

            Statement::Namespace(namespace) => match &namespace {
                namespaces::NamespaceStatement::Unbraced(unbraced) => {
                    for statement in &unbraced.statements {
                        suggestions.append(&mut self.expand(statement, rule));
                    }
                }
                namespaces::NamespaceStatement::Braced(braced) => {
                    for statement in &braced.body.statements {
                        suggestions.append(&mut self.expand(statement, rule));
                    }
                }
            },

            _ => {}
        };
        suggestions
    }
}
