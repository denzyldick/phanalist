use php_parser_rs::parser::ast::{
    classes::ClassMember,
    control_flow::{self, IfStatement},
    functions::MethodBody,
    loops::{WhileStatement, WhileStatementBody},
    BlockStatement, ExpressionStatement, Statement,
};
use serde::{Deserialize, Serialize};

use crate::file::File;
use crate::results::Violation;

pub(crate) static CODE: &str = "E0009";
static DESCRIPTION: &str = "Cyclomatic complexity";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_complexity: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_complexity: 10 }
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
        let mut graph = Graph { n: 0, e: 0, p: 0 };
        let suggestion =
            String::from("This method body is too complex. Make it easier to understand.");

        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::ConcreteMethod(concretemethod) = member {
                    let MethodBody {
                        comments: _,
                        left_brace: _,
                        statements,
                        right_brace: _,
                    } = concretemethod.body.clone();
                    {
                        let graph = calculate_cyclomatic_complexity(statements.clone(), &mut graph);
                        if graph.calculate() > self.settings.max_complexity {
                            violations.push(self.new_violation(
                                file,
                                suggestion.clone(),
                                concretemethod.function,
                            ));
                        }
                    }
                }
            }
        }
        violations
    }
}

#[derive(Debug)]
struct Graph {
    n: i64,
    e: i64,
    p: i64,
}

impl Graph {
    fn calculate(&self) -> i64 {
        self.n - self.e + (2 * self.p)
    }

    fn increase_node(&mut self) {
        self.n += 1;
    }

    fn increase_edge(&mut self) {
        self.e += 1;
    }

    #[allow(dead_code)]
    fn increase_exit_node(&mut self) {
        self.p += 1;
    }

    #[allow(dead_code)]
    fn merge(&mut self, c: &mut Graph) {
        self.n += c.n;
        self.e += c.e;
        self.p += c.p
    }
}

fn calculate_cyclomatic_complexity(
    mut statements: Vec<Statement>,
    graph: &mut Graph,
) -> &mut Graph {
    if !statements.is_empty() {
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

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    pub fn graph_calculate() {
        let g = Graph { n: 8, e: 9, p: 3 };
        assert_eq!(g.calculate(), 5);
    }

    #[test]
    fn complex() {
        let violations = analyze_file_for_rule("e9/complex.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "This method body is too complex. Make it easier to understand.".to_string()
        );
    }

    #[test]
    fn not_complex() {
        let violations = analyze_file_for_rule("e9/not_complex.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
