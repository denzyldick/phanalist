use php_parser_rs::parser::ast::{
    classes::ClassMember,
    control_flow::{self, IfStatement},
    functions::MethodBody,
    loops::WhileStatement,
    BlockStatement, Expression, ExpressionStatement, Statement,
};

use crate::{analyse::Rule, project::Suggestion};

pub struct E009 {}

impl Rule for E009 {
    fn validate(
        &self,
        statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        let mut suggestions = Vec::new();
        match statement {
            Statement::Class(class) => {
                for member in &class.body.members {
                    match member {
                        ClassMember::ConcreteMethod(concretemethod) => {
                            match concretemethod.body.clone() {
                                MethodBody {
                                    comments: _,
                                    left_brace: _,
                                    statements,
                                    right_brace: _,
                                } => {
                                    if calculate_cyclomatic_complexity(statements.clone()) > 10 {
                                        suggestions.push(Suggestion::from(
                            "This method body is too complex. Make it easier to understand."
                                .to_string(),
                            concretemethod.function,
                        ));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        suggestions
    }
}
pub fn calculate_cyclomatic_complexity(mut statements: Vec<Statement>) -> i64 {
    if statements.len() > 0 {
        let statement: Statement = statements.pop().unwrap();
        return match statement {
            Statement::Expression(ExpressionStatement { expression, ending }) => match expression {
                Expression::MethodCall(method) => 1,
                _ => 0,
            },
            Statement::If(IfStatement {
                r#if,
                left_parenthesis,
                condition,
                right_parenthesis,
                body,
            }) => {
                let c = match body {
                    control_flow::IfStatementBody::Block {
                        colon,
                        statements,
                        elseifs,
                        r#else,
                        endif,
                        ending,
                    } => calculate_cyclomatic_complexity(statements),
                    control_flow::IfStatementBody::Statement {
                        statement,
                        elseifs,
                        r#else,
                    } => calculate_cyclomatic_complexity(vec![*statement]),
                };
                c + 1
            }
            Statement::While(WhileStatement {
                r#while,
                left_parenthesis,
                condition,
                right_parenthesis,
                body,
            }) => 1,
            Statement::Block(BlockStatement {
                left_brace,
                statements,
                right_brace,
            }) => calculate_cyclomatic_complexity(statements),
            _ => 0,
        } + calculate_cyclomatic_complexity(statements);
    }
    0
}
