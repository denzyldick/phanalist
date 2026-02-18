use crate::file::File;
use crate::results::Violation;
use mago_ast::ast::class_like::member::ClassLikeMember;
use mago_ast::ast::control_flow::r#if::IfBody;
use mago_ast::ast::r#loop::foreach::ForeachBody;
use mago_ast::ast::r#loop::r#for::ForBody;
use mago_ast::ast::r#loop::r#while::WhileBody;
use mago_ast::*;
use mago_span::HasSpan;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

    fn do_validate(&self, _file: &File) -> bool {
        true
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        };
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    match &method.body {
                        MethodBody::Concrete(block) => {
                            // Base complexity is 1 for the method itself
                            let complexity = 1 + calculate_complexity(&block.statements);

                            if complexity > self.settings.max_complexity {
                                let name = file.interner.lookup(&method.name.value);
                                let suggestion = format!(
                                     "The body of {} method has {} complexity. Make it easier to understand.",
                                     name,
                                     complexity,
                                 );
                                violations.push(self.new_violation(
                                    file,
                                    suggestion,
                                    method.span(),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        violations
    }
}

fn calculate_complexity(statements: &Sequence<Statement>) -> i64 {
    let mut complexity = 0;
    for statement in statements.iter() {
        complexity += calculate_statement_complexity(statement);
    }
    complexity
}

fn calculate_statement_complexity(statement: &Statement) -> i64 {
    let mut complexity = 0;
    match statement {
        Statement::If(if_stmt) => {
            complexity += 1; // if
            match &if_stmt.body {
                IfBody::Statement(body) => {
                    complexity += calculate_statement_complexity(&body.statement);
                    for clause in body.else_if_clauses.iter() {
                        complexity += 1; // elseif
                        complexity += calculate_statement_complexity(&clause.statement);
                    }
                    if let Some(else_clause) = &body.else_clause {
                        complexity += calculate_statement_complexity(&else_clause.statement);
                    }
                }
                IfBody::ColonDelimited(body) => {
                    complexity += calculate_complexity(&body.statements);
                    for clause in body.else_if_clauses.iter() {
                        complexity += 1; // elseif
                        for s in clause.statements.iter() {
                            complexity += calculate_statement_complexity(s);
                        }
                    }
                    if let Some(else_clause) = &body.else_clause {
                        for s in else_clause.statements.iter() {
                            complexity += calculate_statement_complexity(s);
                        }
                    }
                }
            }
        }
        Statement::While(while_stmt) => {
            complexity += 1;
            match &while_stmt.body {
                WhileBody::Statement(body) => {
                    complexity += calculate_statement_complexity(body);
                }
                WhileBody::ColonDelimited(body) => {
                    complexity += calculate_complexity(&body.statements);
                }
            }
        }
        Statement::DoWhile(do_while_stmt) => {
            complexity += 1;
            complexity += calculate_statement_complexity(&do_while_stmt.statement);
        }
        Statement::For(for_stmt) => {
            complexity += 1;
            match &for_stmt.body {
                ForBody::Statement(body) => {
                    complexity += calculate_statement_complexity(body);
                }
                ForBody::ColonDelimited(body) => {
                    complexity += calculate_complexity(&body.statements);
                }
            }
        }
        Statement::Foreach(foreach_stmt) => {
            complexity += 1;
            match &foreach_stmt.body {
                ForeachBody::Statement(body) => {
                    complexity += calculate_statement_complexity(body);
                }
                ForeachBody::ColonDelimited(body) => {
                    complexity += calculate_complexity(&body.statements);
                }
            }
        }
        Statement::Switch(switch_stmt) => {
            let cases = match &switch_stmt.body {
                mago_ast::ast::control_flow::switch::SwitchBody::BraceDelimited(body) => {
                    &body.cases
                }
                mago_ast::ast::control_flow::switch::SwitchBody::ColonDelimited(body) => {
                    &body.cases
                }
            };
            for case in cases.iter() {
                match case {
                    mago_ast::ast::control_flow::switch::SwitchCase::Expression(c) => {
                        complexity += 1;
                        complexity += calculate_complexity(&c.statements);
                    }
                    mago_ast::ast::control_flow::switch::SwitchCase::Default(c) => {
                        // Default case does not increase complexity usually? Or does it?
                        // McCabe says number of branches. Default is the "else".
                        // Usually 'case' adds 1. Default doesn't.
                        complexity += calculate_complexity(&c.statements);
                    }
                }
            }
        }
        Statement::Try(try_stmt) => {
            complexity += calculate_complexity(&try_stmt.block.statements);
            for catch in try_stmt.catch_clauses.iter() {
                complexity += 1;
                complexity += calculate_complexity(&catch.block.statements);
            }
            if let Some(finally) = &try_stmt.finally_clause {
                complexity += calculate_complexity(&finally.block.statements);
            }
        }
        Statement::Block(block) => {
            complexity += calculate_complexity(&block.statements);
        }
        _ => {}
    }
    complexity
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn complex() {
        let violations = analyze_file_for_rule("e9/complex.php", CODE);

        assert!(violations.len().gt(&0));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "The body of complexMethod method has 11 complexity. Make it easier to understand."
                .to_string()
        );
    }

    #[test]
    fn not_complex() {
        let violations = analyze_file_for_rule("e9/not_complex.php", CODE);

        assert!(violations.len().eq(&0));
    }
}
