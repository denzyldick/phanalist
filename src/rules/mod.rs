use colored::Colorize;
use std::collections::HashMap;
use std::default::Default;
use std::error::Error;

use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::Statement;
use serde_json::Value;

use crate::config::Config;
use crate::file::File;
use crate::results::Violation;
use crate::rules::ast_child_statements::AstChildStatements;

mod ast_child_statements;
pub mod e0;
pub mod e1;
pub mod e10;
pub mod e12;
pub mod e13;
pub mod e2;
pub mod e3;
pub mod e4;
pub mod e5;
pub mod e6;
pub mod e7;
pub mod e8;
pub mod e9;

pub trait Rule {
    // Would be a good idea to have default implementation which extracts the code from struct name
    // Haven't found a way to implement it
    fn get_code(&self) -> String;

    fn description(&self) -> String {
        String::from("")
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
        let line = file.lines.get(span.line - 1).unwrap();

        Violation {
            rule: self.get_code(),
            line: String::from(line),
            suggestion,
            span,
        }
    }

    fn flatten_statements_to_validate<'a>(&'a self, statement: &'a Statement) -> Vec<&Statement> {
        let flatten_statements: Vec<&Statement> = Vec::new();

        self.travers_statements_to_validate(flatten_statements, statement)
    }

    fn travers_statements_to_validate<'a>(
        &'a self,
        mut flatten_statements: Vec<&'a Statement>,
        statement: &'a Statement,
    ) -> Vec<&Statement> {
        flatten_statements.push(statement);

        let child_statements: AstChildStatements = match statement {
            Statement::Namespace(statement) => statement.into(),
            Statement::Trait(statement) => statement.into(),
            Statement::Class(statement) => statement.into(),
            Statement::Block(statement) => statement.into(),
            Statement::If(statement) => statement.into(),
            Statement::Switch(statement) => statement.into(),
            Statement::While(statement) => statement.into(),
            Statement::Foreach(statement) => statement.into(),
            Statement::For(statement) => statement.into(),
            Statement::Try(statement) => statement.into(),
            _ => AstChildStatements { statements: vec![] },
        };

        for statement in child_statements.statements {
            flatten_statements.append(&mut self.flatten_statements_to_validate(statement));
        }

        flatten_statements
    }

    fn class_statements_only_to_validate<'a>(
        &'a self,
        mut flatten_statements: Vec<&'a Statement>,
        statement: &'a Statement,
    ) -> Vec<&Statement> {
        if let Statement::Class(_) = &statement {
            flatten_statements.push(statement);
        };

        let child_statements: AstChildStatements = match statement {
            Statement::Namespace(statement) => statement.into(),
            _ => AstChildStatements { statements: vec![] },
        };

        for statement in &child_statements.statements {
            flatten_statements.append(&mut self.flatten_statements_to_validate(statement));
        }

        flatten_statements
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
    add_rule(&mut rules, Box::default() as Box<e12::Rule>);
    add_rule(&mut rules, Box::new(e13::Rule {}));

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
}
