use std::str;

use php_parser_rs::parser::ast::classes::ClassStatement;
use php_parser_rs::parser::ast::control_flow::{IfStatement, IfStatementBody};
use php_parser_rs::parser::ast::identifiers::Identifier;
use php_parser_rs::parser::ast::loops::{
    ForStatementBody, ForeachStatement, ForeachStatementBody, WhileStatementBody,
};
use php_parser_rs::parser::ast::try_block::CatchBlock;
use php_parser_rs::parser::ast::variables::Variable;
use php_parser_rs::parser::ast::{
    namespaces, BlockStatement, Expression, PropertyFetchExpression, Statement, SwitchStatement,
};
use serde::{Deserialize, Serialize};

use crate::file::File;
use crate::results::Violation;

static CODE: &str = "E0011";
static DESCRIPTION: &str = "Service properties";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub include_namespaces: Vec<String>,
    pub exclude_namespaces: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            include_namespaces: vec!["App\\Service\\".to_string()],
            exclude_namespaces: vec![],
        }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Expression(expression) = statement {
            if let Expression::AssignmentOperation(assignment) = &expression.expression {
                if let Expression::PropertyFetch(property) = &assignment.left() {
                    if let Expression::Variable(Variable::SimpleVariable(var)) =
                        &property.target.as_ref()
                    {
                        if str::from_utf8(&var.name).unwrap() == "$this" {
                            let suggestion = format!("Setting service properties leads to issues with Swoole. Trying to set $this->{} property", Self::get_property_identifier(property));
                            violations.push(self.new_violation(file, suggestion, var.span));
                        }
                    }
                }
            }
        }

        violations
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
                    if let php_parser_rs::parser::ast::classes::ClassMember::ConcreteMethod(
                        concrete_method,
                    ) = member
                    {
                        let statements = &concrete_method.body.statements;
                        for statement in statements {
                            expanded_statements.append(&mut self.flatten_statements(statement));
                        }
                    }
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

impl Rule {
    fn get_property_identifier(property: &PropertyFetchExpression) -> String {
        if let Expression::Identifier(Identifier::SimpleIdentifier(simple_id)) =
            property.property.as_ref()
        {
            return simple_id.value.to_string();
        }

        "n/a".to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn define_in_constructor() {
        let violations = analyze_file_for_rule("e11/define_in_constructor.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_in_constructor() {
        let violations = analyze_file_for_rule("e11/set_in_constructor.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_in_method() {
        let violations = analyze_file_for_rule("e11/set_in_method.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "Setting service properties leads to issues with Swoole. Trying to set $this->counter property".to_string()
        );
    }

    #[test]
    fn increment_in_method() {
        let violations = analyze_file_for_rule("e11/increment_in_method.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn set_local_in_method() {
        let violations = analyze_file_for_rule("e11/set_local_in_method.php", CODE);

        assert!(violations.len().eq(&0));
    }

    #[test]
    fn increment_local_in_method() {
        let violations = analyze_file_for_rule("e11/increment_local_in_method.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
