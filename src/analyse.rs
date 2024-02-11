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
use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule;
use crate::rules::{self};

pub struct Analyse {
    rules: HashMap<String, Box<dyn Rule>>,
}

impl Analyse {
    pub fn new(config: &Config) -> Self {
        Self {
            rules: Self::get_active_rules(config),
        }
    }

    fn get_active_rules(config: &Config) -> HashMap<String, Box<dyn Rule>> {
        let active_codes = Self::filter_active_codes(
            rules::all_rules().into_keys().collect(),
            &config.enabled_rules,
            &config.disable_rules,
        );

        let mut active_rules = rules::all_rules();
        active_rules.retain(|code, rule| {
            rule.read_config(config);

            active_codes.contains(code)
        });
        active_rules
    }

    fn filter_active_codes(
        all_codes: Vec<String>,
        enabled: &Vec<String>,
        disabled: &Vec<String>,
    ) -> Vec<String> {
        let mut filtered_codes = all_codes;

        if !enabled.is_empty() {
            filtered_codes.retain(|x| enabled.contains(x));
        }

        if !disabled.is_empty() {
            filtered_codes.retain(|x| !disabled.contains(x));
        }

        filtered_codes
    }

    pub fn analyse(&self, file: &File, statement: &parser::ast::Statement) -> Vec<Violation> {
        let mut suggestions = Vec::new();
        let rules = &self.rules;
        for (_, rule) in rules.iter() {
            suggestions.append(&mut self.expand(statement, file, rule));
        }
        suggestions
    }

    #[allow(clippy::only_used_in_recursion)]
    #[allow(clippy::borrowed_box)]
    fn expand(&self, statement: &Statement, file: &File, rule: &Box<dyn Rule>) -> Vec<Violation> {
        let mut suggestions = Vec::new();
        suggestions.append(&mut rule.validate(file, statement));
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
                        suggestions.append(&mut self.expand(statement, file, rule));
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
                                suggestions.append(&mut self.expand(statement, file, rule));
                            }
                        }
                        php_parser_rs::parser::ast::classes::ClassMember::ConcreteConstructor(
                            concrete_constructor,
                        ) => {
                            let statements = &concrete_constructor.body.statements;

                            for statement in statements {
                                suggestions.append(&mut self.expand(statement, file, rule));
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
                                suggestions.append(&mut self.expand(statement, file, rule));
                            }
                        }
                        IfStatementBody::Statement {
                            statement,
                            elseifs: _,
                            r#else: _,
                        } => suggestions.append(&mut self.expand(statement, file, rule)),
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
                        suggestions.append(&mut self.expand(statement, file, rule));
                    }
                }
                WhileStatementBody::Statement { statement } => {
                    suggestions.append(&mut self.expand(statement, file, rule));
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
                        suggestions.append(&mut self.expand(statement, file, rule))
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
                        suggestions.append(&mut self.expand(statement, file, rule));
                    }
                }
                ForeachStatementBody::Statement { statement } => {
                    suggestions.append(&mut self.expand(statement, file, rule));
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
                        suggestions.append(&mut self.expand(statement, file, rule));
                    }
                }
                ForStatementBody::Statement { statement } => {
                    suggestions.append(&mut self.expand(statement, file, rule))
                }
            },
            Statement::Block(BlockStatement {
                left_brace: _,
                statements,
                right_brace: _,
            }) => {
                for statement in statements {
                    suggestions.append(&mut self.expand(statement, file, rule));
                }
            }

            Statement::Namespace(namespace) => match &namespace {
                namespaces::NamespaceStatement::Unbraced(unbraced) => {
                    for statement in &unbraced.statements {
                        suggestions.append(&mut self.expand(statement, file, rule));
                    }
                }
                namespaces::NamespaceStatement::Braced(braced) => {
                    for statement in &braced.body.statements {
                        suggestions.append(&mut self.expand(statement, file, rule));
                    }
                }
            },

            _ => {}
        };

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_all_codes() -> Vec<String> {
        vec![
            "RULE1".to_string(),
            "RULE2".to_string(),
            "RULE3".to_string(),
            "RULE4".to_string(),
        ]
    }

    fn get_enabled_codes() -> Vec<String> {
        vec![
            "RULE1".to_string(),
            "RULE3".to_string(),
            "RULE103".to_string(),
        ]
    }

    fn get_disabled_codes() -> Vec<String> {
        vec![
            "RULE2".to_string(),
            "RULE3".to_string(),
            "RULE203".to_string(),
        ]
    }

    #[test]
    fn test_filter_active_codes_all_enabled() {
        let all_codes = get_all_codes();
        let active_codes = Analyse::filter_active_codes(all_codes.clone(), &vec![], &vec![]);

        assert_eq!(all_codes, active_codes);
    }

    #[test]
    fn test_filter_active_codes_some_enabled() {
        let all_codes = get_all_codes();
        let enabled_codes = get_enabled_codes();
        let active_codes = Analyse::filter_active_codes(all_codes, &enabled_codes, &vec![]);

        assert_eq!(vec!["RULE1".to_string(), "RULE3".to_string()], active_codes);
    }

    #[test]
    fn test_filter_active_codes_some_disabled() {
        let all_codes = get_all_codes();
        let disabled_codes = get_disabled_codes();
        let active_codes = Analyse::filter_active_codes(all_codes, &vec![], &disabled_codes);

        assert_eq!(vec!["RULE1".to_string(), "RULE4".to_string()], active_codes);
    }

    #[test]
    fn test_filter_active_codes_some_enabled_and_disabled() {
        let all_codes = get_all_codes();
        let disabled_codes = get_disabled_codes();
        let enabled_codes = get_enabled_codes();
        let active_codes = Analyse::filter_active_codes(all_codes, &enabled_codes, &disabled_codes);

        assert_eq!(vec!["RULE1".to_string()], active_codes);
    }
}
