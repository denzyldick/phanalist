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
    /// Paths excluded from scanning entirely, before any rule runs. Directory
    /// prefixes (`var/cache`) or globs (`**/*.generated.php`).
    #[serde(default)]
    pub exclude_paths: Vec<String>,
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
        rules.insert(
            String::from(rules::e10::CODE),
            serde_json::to_value(rules::e10::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e12::CODE),
            serde_json::to_value(rules::e12::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e18::CODE),
            serde_json::to_value(rules::e18::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e19::CODE),
            serde_json::to_value(rules::e19::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e20::CODE),
            serde_json::to_value(rules::e20::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e21::CODE),
            serde_json::to_value(rules::e21::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e22::CODE),
            serde_json::to_value(rules::e22::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e23::CODE),
            serde_json::to_value(rules::e23::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e15::CODE),
            serde_json::to_value(rules::e15::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e16::CODE),
            serde_json::to_value(rules::e16::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e24::CODE),
            serde_json::to_value(rules::e24::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e25::CODE),
            serde_json::to_value(rules::e25::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e26::CODE),
            serde_json::to_value(rules::e26::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e27::CODE),
            serde_json::to_value(rules::e27::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e28::CODE),
            serde_json::to_value(rules::e28::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e29::CODE),
            serde_json::to_value(rules::e29::Settings::default()).unwrap(),
        );
        rules.insert(
            String::from(rules::e30::CODE),
            serde_json::to_value(rules::e30::Settings::default()).unwrap(),
        );

        Config {
            enabled_rules,
            disable_rules,
            rules,
            exclude_paths: vec![],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_paths_defaults_to_empty_when_absent() {
        let yaml = "enabled_rules: []\ndisable_rules: []\nrules: {}\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.exclude_paths.is_empty());
    }

    #[test]
    fn exclude_paths_parsed_from_yaml() {
        let yaml =
            "enabled_rules: []\ndisable_rules: []\nrules: {}\nexclude_paths:\n  - var/cache\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.exclude_paths, vec!["var/cache".to_string()]);
    }
}
