use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::resource::{ResourceClass, ResourceSettings};
use crate::rules;
use crate::verification::{FindingClass, Settings};

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Config {
    pub enabled_rules: Vec<String>,
    pub disable_rules: Vec<String>,
    pub rules: HashMap<String, JsonValue>,
    /// Paths excluded from scanning entirely, before any rule runs. Directory
    /// prefixes (`var/cache`) or globs (`**/*.generated.php`).
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    /// Only report taint findings whose source is request-controlled.
    #[serde(default)]
    pub taint_source_request: bool,
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
        for (code, settings) in [
            (
                rules::e53::CODE,
                serde_json::to_value(rules::e53::Settings::default()).unwrap(),
            ),
            (
                rules::e54::CODE,
                serde_json::to_value(rules::e54::Settings::default()).unwrap(),
            ),
            (
                rules::e55::CODE,
                serde_json::to_value(rules::e55::Settings::default()).unwrap(),
            ),
            (
                rules::e59::CODE,
                serde_json::to_value(rules::e59::Settings::default()).unwrap(),
            ),
            (
                rules::e60::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e61::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e62::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e63::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e64::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e65::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e66::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e67::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e68::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e69::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e70::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e71::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e78::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e79::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e83::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e84::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e87::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e88::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e89::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e90::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e92::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e93::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
            (
                rules::e103::CODE,
                serde_json::to_value(crate::rules::taint_support::Settings::default()).unwrap(),
            ),
        ] {
            rules.insert(String::from(code), settings);
        }

        for class in FindingClass::ALL {
            rules.insert(
                String::from(class.code()),
                serde_json::to_value(Settings::default()).unwrap(),
            );
        }
        for class in ResourceClass::all() {
            rules.insert(
                String::from(class.code()),
                serde_json::to_value(ResourceSettings::default()).unwrap(),
            );
        }

        Config {
            enabled_rules,
            disable_rules,
            rules,
            exclude_paths: vec![],
            taint_source_request: true,
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
