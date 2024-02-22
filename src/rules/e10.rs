use php_parser_rs::parser::ast::{
    classes::ClassMember,
    control_flow::{self, IfStatement},
    functions::MethodBody,
    loops::{WhileStatement, WhileStatementBody},
    BlockStatement, Statement,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::results::Violation;
use crate::{
    file::File,
    traverse::{self, Traverse},
};

pub(crate) static CODE: &str = "E00010";
static DESCRIPTION: &str = "Npath complexity";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    max_paths: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_paths: 200 }
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

    fn set_config(&mut self, json: &Value) {
        if let Ok(settings) = serde_json::from_value(json.to_owned()) {
            self.settings = settings;
        }
    }

    fn callback() {
        let mut violations = Vec::new();
        let mut graph = Graph { e: 0 };
        let suggestion = format!(
            "This method body have more than {} paths. Reduce the amount of paths.",
            self.settings.max_paths
        );
        Self::calculate_npath(statements.iter().collect(), &mut graph);
        if graph.calculate() > self.settings.max_paths {
            violations.push(self.new_violation(
                file,
                suggestion.to_string(),
                concretemethod.function,
            ));
        }
    }
    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();
        let mut graph = Graph { e: 0 };
        let suggestion = format!(
            "This method body have more than {} paths. Reduce the amount of paths.",
            self.settings.max_paths
        );

        let traverse = traverse::Traverse::new(statement.to_owned());
        traverse.find_class_members(false, Self::callback());
        if let Statement::Class(class) = statement {
            for member in &class.body.members {
                if let ClassMember::ConcreteMethod(concretemethod) = member {
                    let MethodBody {
                        comments: _,
                        left_brace: _,
                        statements,
                        right_brace: _,
                    } = &concretemethod.body;
                    {
                        Self::calculate_npath(statements.iter().collect(), &mut graph);
                        if graph.calculate() > self.settings.max_paths {
                            violations.push(self.new_violation(
                                file,
                                suggestion.to_string(),
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

#[derive(Debug, Copy, Clone)]
struct Graph {
    e: i64,
}

impl Graph {
    fn calculate(&self) -> i64 {
        self.e
    }

    fn increase_edge(&mut self) {
        self.e += 1;
    }
}

impl Rule {
    fn calculate_npath(statements: Vec<&Statement>, graph: &mut Graph) {
        for statement in statements {
            Self::calculate_npath_for_statement(statement, graph);
        }
    }

    fn calculate_npath_for_statement(statement: &Statement, graph: &mut Graph) {
        match statement {
            Statement::If(IfStatement {
                r#if: _,
                left_parenthesis: _,
                condition: _,
                right_parenthesis: _,
                body,
            }) => {
                graph.increase_edge();
                match body {
                    control_flow::IfStatementBody::Block {
                        colon: _,
                        statements,
                        elseifs: _,
                        r#else: _,
                        endif: _,
                        ending: _,
                    } => Self::calculate_npath(statements.iter().collect(), graph),
                    control_flow::IfStatementBody::Statement {
                        statement,
                        elseifs: _,
                        r#else: else_statement,
                    } => {
                        Self::calculate_npath(vec![statement.as_ref()], graph);
                        if let Some(e) = else_statement {
                            graph.increase_edge();
                            Self::calculate_npath(vec![e.statement.as_ref()], graph);
                        }
                    }
                }
            }
            Statement::While(WhileStatement {
                r#while: _,
                left_parenthesis: _,
                condition: _,
                right_parenthesis: _,
                body,
            }) => {
                graph.increase_edge();
                match body {
                    WhileStatementBody::Block {
                        colon: _,
                        statements,
                        endwhile: _,
                        ending: _,
                    } => Self::calculate_npath(statements.iter().collect(), graph),
                    WhileStatementBody::Statement { statement } => {
                        Self::calculate_npath(vec![statement.as_ref()], graph);
                    }
                };
            }
            Statement::Block(BlockStatement {
                left_brace: _,
                statements,
                right_brace: _,
            }) => {
                Self::calculate_npath(statements.iter().collect(), graph);
            }
            _ => {}
        };
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    pub fn graph_calculate() {
        let mut g = Graph { e: 0 };
        g.increase_edge();
        g.increase_edge();
        let result = g.calculate();

        assert_eq!(2, result);
    }

    #[test]
    fn complex() {
        let violations = analyze_file_for_rule("e10/npath.php", CODE);
        assert_eq!(
            violations.first().unwrap().suggestion,
            format!(
                "This method body have more than {} paths. Reduce the amount of paths.",
                200
            )
        );
    }
}
