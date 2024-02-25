use std::convert::From;

use php_parser_rs::parser::ast::classes::ClassMember;
use php_parser_rs::parser::ast::classes::ClassStatement;
use php_parser_rs::parser::ast::control_flow::{IfStatement, IfStatementBody};
use php_parser_rs::parser::ast::loops::WhileStatement;
use php_parser_rs::parser::ast::loops::{
    ForStatement, ForStatementBody, ForeachStatement, ForeachStatementBody, WhileStatementBody,
};
use php_parser_rs::parser::ast::namespaces::NamespaceStatement;
use php_parser_rs::parser::ast::traits::{TraitMember, TraitStatement};
use php_parser_rs::parser::ast::{BlockStatement, Statement, SwitchStatement};

pub struct AstChildStatements<'a> {
    pub statements: Vec<&'a Statement>,
}

impl<'a> From<&'a NamespaceStatement> for AstChildStatements<'a> {
    fn from(statement: &'a NamespaceStatement) -> Self {
        let mut statements = vec![];

        match statement {
            NamespaceStatement::Unbraced(unbraced) => {
                for statement in &unbraced.statements {
                    statements.push(statement);
                }
            }
            NamespaceStatement::Braced(braced) => {
                for statement in &braced.body.statements {
                    statements.push(statement);
                }
            }
        }

        Self { statements }
    }
}

impl<'a> From<&'a TraitStatement> for AstChildStatements<'a> {
    fn from(statement: &'a TraitStatement) -> Self {
        let mut statements = vec![];

        for member in &statement.body.members {
            if let TraitMember::ConcreteMethod(concrete_method) = member {
                for body_statement in &concrete_method.body.statements {
                    statements.push(body_statement)
                }
            }
        }

        Self { statements }
    }
}

impl<'a> From<&'a ClassStatement> for AstChildStatements<'a> {
    fn from(statement: &'a ClassStatement) -> Self {
        let mut statements = vec![];

        for member in &statement.body.members {
            match member {
                ClassMember::ConcreteMethod(method) => {
                    for body_statement in &method.body.statements {
                        statements.push(body_statement);
                    }
                }
                ClassMember::ConcreteConstructor(constructor) => {
                    for body_statement in &constructor.body.statements {
                        statements.push(body_statement);
                    }
                }
                _ => {}
            }
        }

        Self { statements }
    }
}

impl<'a> From<&'a BlockStatement> for AstChildStatements<'a> {
    fn from(statement: &'a BlockStatement) -> Self {
        let mut statements = vec![];

        for block_statement in &statement.statements {
            statements.push(block_statement);
        }

        Self { statements }
    }
}

impl<'a> From<&'a IfStatement> for AstChildStatements<'a> {
    fn from(if_statement: &'a IfStatement) -> Self {
        let mut child_statements = vec![];

        match &if_statement.body {
            IfStatementBody::Block { statements, .. } => {
                for statement in statements {
                    child_statements.push(statement);
                }
            }
            IfStatementBody::Statement { statement, .. } => {
                child_statements.push(statement);
            }
        };

        Self {
            statements: child_statements,
        }
    }
}

impl<'a> From<&'a WhileStatement> for AstChildStatements<'a> {
    fn from(while_statement: &'a WhileStatement) -> Self {
        let mut child_statements = vec![];

        match &while_statement.body {
            WhileStatementBody::Block { statements, .. } => {
                for statement in statements {
                    child_statements.push(statement);
                }
            }
            WhileStatementBody::Statement { statement } => {
                child_statements.push(statement);
            }
        }

        Self {
            statements: child_statements,
        }
    }
}

impl<'a> From<&'a SwitchStatement> for AstChildStatements<'a> {
    fn from(statement: &'a SwitchStatement) -> Self {
        let mut statements = vec![];

        for case in &statement.cases {
            for statement in &case.body {
                statements.push(statement);
            }
        }

        Self { statements }
    }
}

impl<'a> From<&'a ForeachStatement> for AstChildStatements<'a> {
    fn from(foreach_statement: &'a ForeachStatement) -> Self {
        let mut child_statements = vec![];

        match &foreach_statement.body {
            ForeachStatementBody::Block { statements, .. } => {
                for statement in statements {
                    child_statements.push(statement);
                }
            }
            ForeachStatementBody::Statement { statement } => {
                child_statements.push(statement);
            }
        }

        Self {
            statements: child_statements,
        }
    }
}

impl<'a> From<&'a ForStatement> for AstChildStatements<'a> {
    fn from(for_statement: &'a ForStatement) -> Self {
        let mut child_statements = vec![];

        match &for_statement.body {
            ForStatementBody::Block { statements, .. } => {
                for statement in statements {
                    child_statements.push(statement);
                }
            }
            ForStatementBody::Statement { statement } => {
                child_statements.push(statement);
            }
        }

        Self {
            statements: child_statements,
        }
    }
}
