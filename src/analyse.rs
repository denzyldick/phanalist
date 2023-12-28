use crate::project::{File, Suggestion};
use php_parser_rs::parser;
use php_parser_rs::parser::ast::classes::ClassStatement;
use php_parser_rs::parser::ast::control_flow::{IfStatement, IfStatementBody};
use php_parser_rs::parser::ast::loops::{
    ForStatementBody, ForeachStatement, ForeachStatementBody, WhileStatementBody,
};
use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::{namespaces, BlockStatement, Statement, SwitchStatement};
use std::collections::HashMap;

pub trait Rule {
    fn validate(&self, statement: &Statement) -> Vec<Suggestion>;
    fn set_file(&self, file: File) {}
}

pub struct Analyse {
    rules: HashMap<String, Box<dyn Rule>>,
    file: File,
}

use crate::rules::{self, E0010};
impl Analyse {
    pub fn new(disable: Vec<String>, file: File) -> Self {
        let mut rules = HashMap::new();
        // @todo refactor this code below
        if disable.contains(&"E001".to_string()) == false {
            rules.insert(
                "E001".to_string(),
                Box::new(rules::E001::E001 {}) as Box<dyn Rule>,
            );
        }

        if disable.contains(&"E002".to_string()) == false {
            rules.insert(
                "E002".to_string(),
                Box::new(rules::E002::E002 {}) as Box<dyn Rule>,
            );
        }

        if disable.contains(&"E003".to_string()) == false {
            rules.insert(
                "E003".to_string(),
                Box::new(rules::E003::E003 {}) as Box<dyn Rule>,
            );
        }

        if disable.contains(&"E004".to_string()) == false {
            rules.insert(
                "E004".to_string(),
                Box::new(rules::E004::E004 {}) as Box<dyn Rule>,
            );
        }
        if disable.contains(&"E005".to_string()) == false {
            rules.insert(
                "E005".to_string(),
                Box::new(rules::E005::E005 {}) as Box<dyn Rule>,
            );
        }

        if disable.contains(&"E006".to_string()) == false {
            rules.insert(
                "E006".to_string(),
                Box::new(rules::E006::E006 {}) as Box<dyn Rule>,
            );
        }

        if disable.contains(&"E007".to_string()) == false {
            rules.insert(
                "E007".to_string(),
                Box::new(rules::E007::E007 {}) as Box<dyn Rule>,
            );
        }

        if disable.contains(&"E008".to_string()) == false {
            rules.insert(
                "E008".to_string(),
                Box::new(rules::E008::E008 {}) as Box<dyn Rule>,
            );
        }

        if disable.contains(&"E009".to_string()) == false {
            rules.insert(
                "E009".to_string(),
                Box::new(rules::E009::E009 {}) as Box<dyn Rule>,
            );
        }
        if disable.contains(&"E0010".to_string()) == false {
            rules.insert(
                "E0010".to_string(),
                Box::new(E0010::E0010::new(file.clone())) as Box<dyn Rule>,
            );
        }
        if disable.contains(&"E0011".to_string()) == false {
            rules.insert(
                "E0011".to_string(),
                Box::new(rules::E0011::E0011 {}) as Box<dyn Rule>,
            );
        }
        let analyse = Self { rules, file };
        analyse
    }

    pub fn statement(&self, statement: parser::ast::Statement) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        let rules = &self.rules;
        for (_, rule) in rules.into_iter() {
            suggestions.append(&mut self.expand(&statement, rule));
        }
        suggestions
    }

    fn expand(&self, statement: &Statement, rule: &Box<dyn Rule>) -> Vec<Suggestion> {
        let mut suggestions = Vec::new();
        suggestions.append(&mut rule.validate(statement));
        match statement {
            Statement::Try(s) => {
                for catch in &s.catches {
                    match catch {
                        CatchBlock {
                            start: _,
                            end: _,
                            types: _,
                            var: _,
                            body,
                        } => {
                            for statement in body {
                                suggestions.append(&mut self.expand(statement, rule));
                            }
                        }
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
            Statement::If(if_statement) => match if_statement {
                IfStatement {
                    r#if: _,
                    left_parenthesis: _,
                    condition: _,
                    right_parenthesis: _,
                    body,
                } => {
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
                                suggestions.append(&mut &mut self.expand(statement, rule));
                            }
                        }
                        IfStatementBody::Statement {
                            statement,
                            elseifs: _,
                            r#else: _,
                        } => suggestions.append(&mut self.expand(statement, rule)),
                    };
                }
            },
            Statement::While(while_statement) => match &while_statement.body {
                WhileStatementBody::Block {
                    colon: _,
                    statements,
                    endwhile: _,
                    ending: _,
                } => {
                    for statement in statements {
                        suggestions.append(&mut self.expand(&statement, rule));
                    }
                }
                WhileStatementBody::Statement { statement } => {
                    suggestions.append(&mut self.expand(&statement, rule));
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
                        suggestions.append(&mut self.expand(&statement, rule));
                    }
                }
                ForStatementBody::Statement { statement } => {
                    suggestions.append(&mut self.expand(&statement, rule))
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
