use php_parser_rs::parser::ast::{
    classes::ClassMember,
    control_flow::{self, IfStatement},
    functions::MethodBody,
    loops::{WhileStatement, WhileStatementBody},
    BlockStatement, ExpressionStatement, Statement,
};

use crate::{analyse::Rule, project::Suggestion};

pub struct E009 {}

struct Graph {
    n: i64,
    e: i64,
    p: i64,
}

trait NewTrait {
    fn new() -> Self;
    fn calculate(&self) -> i64;

    fn increase_node(&mut self);

    fn increase_edge(&mut self);

    fn increase_exit_node(&mut self);

    fn merge(&mut self, c: &mut Graph);
}

impl NewTrait for Graph {
    fn new() -> Self {
        Self { n: 0, e: 0, p: 0 }
    }
    fn calculate(&self) -> i64 {
        self.n - self.e + (2 * self.p)
    }

    fn increase_node(&mut self) {
        self.n = self.n + 1;
    }

    fn increase_edge(&mut self) {
        self.e = self.e + 1;
    }

    fn increase_exit_node(&mut self) {
        self.p = self.p + 1;
    }

    fn merge(&mut self, c: &mut Graph) {
        self.n = self.n + c.n;
        self.e = self.e + c.e;
        self.p = self.p + c.p
    }
}
#[test]
pub fn calculate() {
    let g = Graph { n: 8, e: 9, p: 3 };
    assert_eq!(g.calculate(), 5);
}
impl Rule for E009 {
    fn validate(
        &self,
        statement: &php_parser_rs::parser::ast::Statement,
    ) -> Vec<crate::project::Suggestion> {
        let mut suggestions = Vec::new();
        let mut graph = Graph { n: 0, e: 0, p: 0 };
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
                                    let graph = calculate_cyclomatic_complexity(
                                        statements.clone(),
                                        &mut graph,
                                    );

                                    if graph.calculate() > 10 {
                                        suggestions.push(Suggestion::from(
                            "This method body is too complex. Make it easier to understand."
                                .to_string(),
                            concretemethod.function,
                                                "E009".to_string()
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
fn calculate_cyclomatic_complexity(
    mut statements: Vec<Statement>,
    graph: &mut Graph,
) -> &mut Graph {
    if statements.len() > 0 {
        let statement: Statement = statements.pop().unwrap();
        return match statement {
            Statement::Expression(ExpressionStatement {
                expression: _,
                ending: _,
            }) => {
                graph.increase_edge();
                graph
            }
            Statement::If(IfStatement {
                r#if: _,
                left_parenthesis: _,
                condition: _,
                right_parenthesis: _,
                body,
            }) => {
                graph.increase_node();
                let c = match body {
                    control_flow::IfStatementBody::Block {
                        colon: _,
                        statements,
                        elseifs: _,
                        r#else: _,
                        endif: _,
                        ending: _,
                    } => calculate_cyclomatic_complexity(statements, graph),
                    control_flow::IfStatementBody::Statement {
                        statement,
                        elseifs: _,
                        r#else: else_statement,
                    } => {
                        graph.increase_node();
                        let g = calculate_cyclomatic_complexity(vec![*statement], graph);
                        match else_statement {
                            Some(e) => calculate_cyclomatic_complexity(vec![*e.statement], g),
                            None => g,
                        }
                    }
                };
                c
            }
            Statement::While(WhileStatement {
                r#while: _,
                left_parenthesis: _,
                condition: _,
                right_parenthesis: _,
                body,
            }) => {
                graph.increase_node();

                match body {
                    WhileStatementBody::Block {
                        colon: _,
                        statements,
                        endwhile: _,
                        ending: _,
                    } => calculate_cyclomatic_complexity(statements, graph),
                    WhileStatementBody::Statement { statement } => {
                        calculate_cyclomatic_complexity(vec![*statement], graph)
                    }
                };
                graph
            }
            Statement::Block(BlockStatement {
                left_brace: _,
                statements,
                right_brace: _,
            }) => calculate_cyclomatic_complexity(statements, graph),
            _ => {
                graph.increase_edge();
                graph
            }
        };
    }
    graph
}
