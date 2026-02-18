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

pub(crate) static CODE: &str = "E0010";
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
                            let npath = calculate_npath(&block.statements);
                            if npath > self.settings.max_paths {
                                let name = file.interner.lookup(&method.name.value);
                                let suggestion = format!(
                                     "The body of {} method has {} paths. Reduce the amount of paths.",
                                     name,
                                     npath,
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

fn calculate_npath(statements: &Sequence<Statement>) -> i64 {
    let mut npath = 0;
    for statement in statements.iter() {
        npath += calculate_statement_npath(statement);
    }
    npath
}

fn calculate_statement_npath(statement: &Statement) -> i64 {
    let mut npath = 0;
    match statement {
        Statement::If(if_stmt) => {
            npath += 1;
            match &if_stmt.body {
                IfBody::Statement(body) => {
                    npath += calculate_statement_npath(&body.statement);
                    for clause in body.else_if_clauses.iter() {
                        npath += 1;
                        npath += calculate_statement_npath(&clause.statement);
                    }
                    if let Some(else_clause) = &body.else_clause {
                        npath += 1;
                        npath += calculate_statement_npath(&else_clause.statement);
                    }
                }
                IfBody::ColonDelimited(body) => {
                    npath += calculate_npath(&body.statements);
                    for clause in body.else_if_clauses.iter() {
                        npath += 1; // elseif
                        for s in clause.statements.iter() {
                            npath += calculate_statement_npath(s);
                        }
                    }
                    if let Some(else_clause) = &body.else_clause {
                        npath += 1;
                        for s in else_clause.statements.iter() {
                            npath += calculate_statement_npath(s);
                        }
                    }
                }
            }
        }
        Statement::While(while_stmt) => {
            npath += 1;
            match &while_stmt.body {
                WhileBody::Statement(body) => {
                    npath += calculate_statement_npath(body);
                }
                WhileBody::ColonDelimited(body) => {
                    npath += calculate_npath(&body.statements);
                }
            }
        }
        Statement::DoWhile(do_while_stmt) => {
            npath += 1;
            npath += calculate_statement_npath(&do_while_stmt.statement);
        }
        Statement::For(for_stmt) => {
            npath += 1;
            match &for_stmt.body {
                ForBody::Statement(body) => {
                    npath += calculate_statement_npath(body);
                }
                ForBody::ColonDelimited(body) => {
                    npath += calculate_npath(&body.statements);
                }
            }
        }
        Statement::Foreach(foreach_stmt) => {
            npath += 1;
            match &foreach_stmt.body {
                ForeachBody::Statement(body) => {
                    npath += calculate_statement_npath(body);
                }
                ForeachBody::ColonDelimited(body) => {
                    npath += calculate_npath(&body.statements);
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
                        npath += 1;
                        npath += calculate_npath(&c.statements);
                    }
                    mago_ast::ast::control_flow::switch::SwitchCase::Default(c) => {
                        npath += calculate_npath(&c.statements);
                    }
                }
            }
        }
        Statement::Try(try_stmt) => {
            npath += calculate_npath(&try_stmt.block.statements);
            for catch in try_stmt.catch_clauses.iter() {
                npath += 1;
                npath += calculate_npath(&catch.block.statements);
            }
            if let Some(finally) = &try_stmt.finally_clause {
                npath += calculate_npath(&finally.block.statements);
            }
        }
        Statement::Block(block) => {
            npath += calculate_npath(&block.statements);
        }
        _ => {}
    }
    npath
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn complex() {
        let violations = analyze_file_for_rule("e10/npath.php", CODE);
        assert!(violations.len().eq(&1));
        assert_eq!(
            violations.first().unwrap().suggestion,
            "The body of tooManyPaths method has 319 paths. Reduce the amount of paths."
                .to_string()
        );
    }
}
