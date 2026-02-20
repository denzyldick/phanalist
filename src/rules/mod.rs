use colored::Colorize;
use std::collections::HashMap;
// use std::default::Default;
// use indicatif::ProgressBar;
// use jwalk::WalkDir;
use std::error::Error;
use std::fs;
use std::path::Path;

use mago_ast::ast::control_flow::r#if::IfBody;
use mago_ast::ast::r#loop::foreach::ForeachBody;
use mago_ast::ast::r#loop::r#for::ForBody;
use mago_ast::ast::r#loop::r#while::WhileBody;
// use mago_ast::Program;
use mago_ast::Statement;
use mago_span::Span;
use serde_json::Value;

use crate::config::Config;
use crate::file::File;
use crate::results::Violation;
// use crate::rules::ast_child_statements::AstChildStatements;

// mod ast_child_statements;
pub mod e0;
pub mod e1;
pub mod e10;
pub mod e11;
pub mod e12;
pub mod e13;
pub mod e14;
pub mod e2;
// pub mod e1;
// pub mod e2;
pub mod e3;
pub mod e4;
pub mod e5;
pub mod e6;
pub mod e7;
pub mod e8;
pub mod e9;

pub trait Rule {
    // Optional hook for cross-file type resolution or indexing.
    // Called once for every file before main validation pass.
    fn index_file(&self, _file: &File) {}

    // Would be a good idea to have default implementation which extracts the code from struct name
    // Haven't found a way to implement it
    fn get_code(&self) -> String;

    fn description(&self) -> String {
        String::from("")
    }
    // Every rule has a detailed explenation.
    // They are writting in markdown and are located in
    // the examples directory.
    fn get_detailed_explanation(&self) -> Option<String> {
        let code = self.get_code().replace("000", "");
        let c: Vec<char> = code.chars().collect();
        let rule_number = c.get(1).unwrap();

        let markdown = format!("./src/rules/examples/e{}/e{}.md", rule_number, rule_number);
        let path = Path::new(&markdown);

        if path.exists() {
            let text = fs::read_to_string(path);
            if let Ok(text) = text {
                return Some(text.to_string());
            }
        }
        None
    }

    fn set_config(&mut self, _json: &Value) {}

    fn read_config(&mut self, config: &Config) {
        let code = self.get_code();
        if let Some(rule_config) = config.rules.get(&code) {
            self.set_config(rule_config);
        }
    }

    fn output_error(&self, e: Box<dyn Error>) {
        println!(
            "{}",
            format!(
                "Unable to parse config for rule #{}, so default values will be used. Parsing error: {}",
                self.get_code(),
                e
            )
            .red()
            .bold()
        );
    }

    fn do_validate(&self, file: &File) -> bool {
        file.get_fully_qualified_name().is_some()
    }

    fn validate(&self, file: &File, statement: &Statement) -> Vec<Violation>;

    fn new_violation(&self, file: &File, suggestion: String, span: Span) -> Violation {
        let (line, start_line, start_column, end_line, end_column) =
            if let Ok(source) = file.source_manager.load(&span.start.source) {
                let start_line = source.line_number(span.start.offset);
                let start_column = source.column_number(span.start.offset);
                let end_line = source.line_number(span.end.offset);
                let end_column = source.column_number(span.end.offset);
                (
                    start_line.to_string(),
                    start_line,
                    start_column,
                    end_line,
                    end_column,
                )
            } else {
                (String::from(""), 0, 0, 0, 0)
            };

        Violation {
            rule: self.get_code(),
            line,
            suggestion,
            // span,
            start_line,
            start_column,
            end_line,
            end_column,
        }
    }

    fn flatten_statements_to_validate<'a>(&'a self, statement: &'a Statement) -> Vec<&Statement> {
        let mut flatten_statements: Vec<&Statement> = Vec::new();
        self.travers_statements_to_validate(&mut flatten_statements, statement);
        flatten_statements
    }

    fn travers_statements_to_validate<'a>(
        &'a self,
        flatten_statements: &mut Vec<&'a Statement>,
        statement: &'a Statement,
    ) {
        flatten_statements.push(statement);

        match statement {
            Statement::Block(block) => {
                for s in block.statements.iter() {
                    self.travers_statements_to_validate(flatten_statements, s);
                }
            }
            Statement::If(if_stmt) => match &if_stmt.body {
                IfBody::Statement(body) => {
                    self.travers_statements_to_validate(flatten_statements, &body.statement);
                    for clauses in body.else_if_clauses.iter() {
                        self.travers_statements_to_validate(flatten_statements, &clauses.statement);
                    }
                    if let Some(else_clause) = &body.else_clause {
                        self.travers_statements_to_validate(
                            flatten_statements,
                            &else_clause.statement,
                        );
                    }
                }
                IfBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        self.travers_statements_to_validate(flatten_statements, s);
                    }
                    for clauses in body.else_if_clauses.iter() {
                        for s in clauses.statements.iter() {
                            self.travers_statements_to_validate(flatten_statements, s);
                        }
                    }
                    if let Some(else_clause) = &body.else_clause {
                        for s in else_clause.statements.iter() {
                            self.travers_statements_to_validate(flatten_statements, s);
                        }
                    }
                }
            },
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(body) => {
                    self.travers_statements_to_validate(flatten_statements, body);
                }
                WhileBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        self.travers_statements_to_validate(flatten_statements, s);
                    }
                }
            },
            Statement::DoWhile(do_while_stmt) => {
                self.travers_statements_to_validate(flatten_statements, &do_while_stmt.statement);
            }
            Statement::Foreach(foreach_stmt) => match &foreach_stmt.body {
                ForeachBody::Statement(body) => {
                    self.travers_statements_to_validate(flatten_statements, body);
                }
                ForeachBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        self.travers_statements_to_validate(flatten_statements, s);
                    }
                }
            },
            Statement::For(for_stmt) => match &for_stmt.body {
                ForBody::Statement(body) => {
                    self.travers_statements_to_validate(flatten_statements, body);
                }
                ForBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        self.travers_statements_to_validate(flatten_statements, s);
                    }
                }
            },
            Statement::Try(try_stmt) => {
                for s in try_stmt.block.statements.iter() {
                    self.travers_statements_to_validate(flatten_statements, s);
                }
                for catch in try_stmt.catch_clauses.iter() {
                    for s in catch.block.statements.iter() {
                        self.travers_statements_to_validate(flatten_statements, s);
                    }
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    for s in finally.block.statements.iter() {
                        self.travers_statements_to_validate(flatten_statements, s);
                    }
                }
            }
            Statement::Namespace(namespace) => {
                for s in namespace.statements().iter() {
                    self.travers_statements_to_validate(flatten_statements, s);
                }
            }
            Statement::Class(class) => {
                for member in class.members.iter() {
                    if let mago_ast::ast::class_like::member::ClassLikeMember::Method(method) =
                        member
                    {
                        match &method.body {
                            mago_ast::MethodBody::Concrete(block) => {
                                for s in block.statements.iter() {
                                    self.travers_statements_to_validate(flatten_statements, s);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Statement::Interface(interface) => {
                for member in interface.members.iter() {
                    if let mago_ast::ast::class_like::member::ClassLikeMember::Method(method) =
                        member
                    {
                        match &method.body {
                            mago_ast::MethodBody::Concrete(block) => {
                                for s in block.statements.iter() {
                                    self.travers_statements_to_validate(flatten_statements, s);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Statement::Trait(t) => {
                for member in t.members.iter() {
                    if let mago_ast::ast::class_like::member::ClassLikeMember::Method(method) =
                        member
                    {
                        match &method.body {
                            mago_ast::MethodBody::Concrete(block) => {
                                for s in block.statements.iter() {
                                    self.travers_statements_to_validate(flatten_statements, s);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Statement::Enum(e) => {
                for member in e.members.iter() {
                    if let mago_ast::ast::class_like::member::ClassLikeMember::Method(method) =
                        member
                    {
                        match &method.body {
                            mago_ast::MethodBody::Concrete(block) => {
                                for s in block.statements.iter() {
                                    self.travers_statements_to_validate(flatten_statements, s);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn do_validate_namespace(
    ns: String,
    include: &Vec<String>,
    exclude: &Vec<String>,
) -> bool {
    for exclude_ns in exclude {
        if ns.contains(exclude_ns) {
            return false;
        }
    }

    if !include.is_empty() {
        for include_ns in include {
            if ns.contains(include_ns) {
                return true;
            }
        }

        return false;
    }

    true
}

fn add_rule(rules: &mut HashMap<String, Box<dyn Rule>>, rule: Box<dyn Rule>) {
    rules.insert(rule.get_code(), rule as Box<dyn Rule>);
}

pub fn all_rules() -> HashMap<String, Box<dyn Rule>> {
    let mut rules: HashMap<String, Box<dyn Rule>> = HashMap::new();

    add_rule(&mut rules, Box::new(e1::Rule {}));
    add_rule(&mut rules, Box::new(e2::Rule {}));
    add_rule(&mut rules, Box::new(e3::Rule {}));
    add_rule(&mut rules, Box::new(e4::Rule {}));
    add_rule(&mut rules, Box::new(e5::Rule {}));
    add_rule(&mut rules, Box::new(e6::Rule {}));
    add_rule(&mut rules, Box::default() as Box<e7::Rule>);
    add_rule(&mut rules, Box::new(e8::Rule {}));
    add_rule(&mut rules, Box::default() as Box<e9::Rule>);
    add_rule(&mut rules, Box::default() as Box<e10::Rule>);
    add_rule(&mut rules, Box::new(e11::Rule {}));
    add_rule(&mut rules, Box::default() as Box<e12::Rule>);
    add_rule(&mut rules, Box::new(e13::Rule {}));
    add_rule(&mut rules, Box::new(e14::Rule::default()));

    rules
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::analyse::Analyse;

    use super::*;

    pub(crate) fn analyze_file_for_rule(path: &str, rule_code: &str) -> Vec<Violation> {
        let path = PathBuf::from(format!("./src/rules/examples/{path}"));
        let content = fs::read_to_string(&path).unwrap();
        let mut file = File::new(path, content);

        let config = Config {
            enabled_rules: vec![rule_code.to_string()],
            ..Default::default()
        };
        let analyse = Analyse::new(&config);

        analyse.analyse_file(&mut file)
    }

    fn get_ns() -> String {
        "App\\Service\\Search".to_string()
    }

    #[test]
    fn do_validate_namespace_empty_include_and_exclude() {
        assert!(do_validate_namespace(get_ns(), &vec![], &vec![]));
    }

    #[test]
    fn do_validate_namespace_include_contains() {
        assert!(do_validate_namespace(
            get_ns(),
            &vec!["\\Service\\".to_string()],
            &vec![]
        ));
    }

    #[test]
    fn do_validate_namespace_include_not_contains() {
        assert!(!do_validate_namespace(
            get_ns(),
            &vec!["\\Service2\\".to_string()],
            &vec![]
        ));
    }

    #[test]
    fn do_validate_namespace_exclude_contains() {
        assert!(!do_validate_namespace(
            get_ns(),
            &vec![],
            &vec!["\\Service\\".to_string()],
        ));
    }

    #[test]
    fn do_validate_namespace_exclude_not_contains() {
        assert!(do_validate_namespace(
            get_ns(),
            &vec![],
            &vec!["\\Service2\\".to_string()],
        ));
    }

    #[test]
    fn do_validate_namespace_include_contains_exclude_contains() {
        let namespaces = &vec!["\\Service\\".to_string()];
        assert!(!do_validate_namespace(get_ns(), namespaces, namespaces));
    }

    #[test]
    fn validate_markdown() {
        let rule = e1::Rule {};

        let markdown = rule.get_detailed_explanation().unwrap();

        let e1_markdown = fs::read_to_string(Path::new("./src/rules/examples/e1/e1.md")).unwrap();

        assert_eq!(e1_markdown, markdown);
    }
}
