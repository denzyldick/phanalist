use php_parser_rs::parser::ast::Statement;
use serde_json::Value;
use std::collections::HashMap;
use std::default::Default;

use crate::config::Config;
use crate::project::Suggestion;

pub mod e001;
pub mod e0010;
pub mod e0011;
pub mod e002;
pub mod e003;
pub mod e004;
pub mod e005;
pub mod e006;
pub mod e007;
pub mod e008;
pub mod e009;

pub trait Rule {
    // Would be a good idea to have default implementation which extracts the code from struct name
    // Haven't found a way to implement it
    fn get_code(&self) -> String;

    fn description(&self) -> String {
        String::from("")
    }

    fn set_config(&mut self, _json: &Value) {}

    fn read_config(&mut self, config: Config) {
        let code = self.get_code();
        if let Some(rule_config) = config.rules.get(&code) {
            self.set_config(rule_config);
        }
    }

    fn validate(&self, statement: &Statement) -> Vec<Suggestion>;
}

fn add_rule(rules: &mut HashMap<String, Box<dyn Rule>>, rule: Box<dyn Rule>) {
    rules.insert(rule.get_code(), rule as Box<dyn Rule>);
}

pub fn all_rules() -> HashMap<String, Box<dyn Rule>> {
    let mut rules: HashMap<String, Box<dyn Rule>> = HashMap::new();

    add_rule(&mut rules, Box::new(e001::E001 {}));
    add_rule(&mut rules, Box::new(e002::E002 {}));
    add_rule(&mut rules, Box::new(e003::E003 {}));
    add_rule(&mut rules, Box::new(e004::E004 {}));
    add_rule(&mut rules, Box::new(e005::E005 {}));
    add_rule(&mut rules, Box::new(e006::E006 {}));
    add_rule(
        &mut rules,
        Box::new(e007::Rule {
            ..Default::default()
        }),
    );
    add_rule(&mut rules, Box::new(e008::E008 {}));
    add_rule(&mut rules, Box::new(e009::E009 {}));
    add_rule(&mut rules, Box::new(e0010::E0010 {}));
    add_rule(&mut rules, Box::new(e0011::E0011 {}));

    rules
}
