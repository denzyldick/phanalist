use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::rules;

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Config {
    pub enabled_rules: Vec<String>,
    pub disable_rules: Vec<String>,
    pub rules: HashMap<String, JsonValue>,
}

impl Default for Config {
    fn default() -> Self {
        let enabled_rules: Vec<String> = vec![];
        let disable_rules: Vec<String> = vec![];

        let mut rules = HashMap::new();
        rules.insert(
            String::from(rules::e7::CODE),
            serde_json::to_value(rules::e7::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e9::CODE),
            serde_json::to_value(rules::e9::Settings::default()).unwrap(),
        );

        Config {
            enabled_rules,
            disable_rules,
            rules,
        }
    }
}

impl Config {
    pub(crate) fn save(&self, path: &PathBuf) -> std::io::Result<()> {
        let t = serde_yaml::to_string(&self).unwrap();

        let mut file = match std::fs::File::create(path) {
            Ok(f) => f,
            Err(e) => {
                return Err(e);
            }
        };

        file.write_all(t.as_bytes())
    }
}
