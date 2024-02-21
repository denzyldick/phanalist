use std::collections::HashMap;
use std::default::Default;

use php_parser_rs::lexer::token::Span;
use php_parser_rs::parser::ast::Statement;
use serde_json::Value;

use crate::config::Config;
use crate::file::File;
use crate::results::Violation;

pub mod e0;
pub mod e1;
pub mod e10;
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

    rules
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::analyse::Analyse;
    use crate::project::Project;

    use super::*;

    pub(crate) fn analyze_file_for_rule(path: &str, rule_code: &str) -> Vec<Violation> {
        let path = PathBuf::from(format!("./src/rules/examples/{path}"));
        let content = fs::read_to_string(&path).unwrap();
        let file = File::new(path, content);

        let config = Config {
            enabled_rules: vec![rule_code.to_string()],
            ..Default::default()
        };
        let analyse = Analyse::new(&config);

        let project = Project {};
        project.analyse_file(&file, &analyse)
    }
}
