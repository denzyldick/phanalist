use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::outputs::Format;
use crate::resource::{ResourceClass, ResourceSettings};
use crate::rules;
use crate::verification::{FindingClass, Settings};

/// Where automatically generated regression tests are written. Phanalist
/// never executes generated tests or user PHP code.
#[derive(Serialize, Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct TestGenerationConfig {
    #[serde(default = "default_test_output_dir")]
    pub output_dir: String,
}

impl Default for TestGenerationConfig {
    fn default() -> Self {
        Self {
            output_dir: default_test_output_dir(),
        }
    }
}

fn default_test_output_dir() -> String {
    "tests/phanalist-generated".to_string()
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct Config {
    pub enabled_rules: Vec<String>,
    pub disable_rules: Vec<String>,
    pub rules: HashMap<String, JsonValue>,
    /// Paths excluded from scanning entirely, before any rule runs. Directory
    /// prefixes (`var/cache`) or globs (`**/*.generated.php`).
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    /// Report all tainted findings, including parameter-derived leads.
    ///
    /// Defaults to `true`: the deeper pass is always on and non-request-rooted
    /// flows are tiered as low-confidence leads. Set this to `false` (or pass
    /// `--taint-source-request`) to hide leads and keep only flows reachable
    /// from request input.
    #[serde(default = "default_taint_source_all")]
    pub taint_source_all: bool,
    /// Opt-in generation of PHPUnit tests from Z3 counterexamples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_generation: Option<TestGenerationConfig>,
    /// Directories to scan when `--src` is not supplied.
    #[serde(default = "default_paths")]
    pub paths: Vec<String>,
    /// Output format used when `--output-format` is not supplied.
    #[serde(default)]
    pub output_format: Format,
    /// Print only the summary by default. The CLI flag can turn this on; config
    /// users can set it back to `false`.
    #[serde(default)]
    pub summary_only: bool,
    /// Baseline path used when `--use-baseline` is not supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_path: Option<String>,
}

fn default_paths() -> Vec<String> {
    vec!["./src".to_string()]
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
            taint_source_all: true,
            test_generation: None,
            paths: default_paths(),
            output_format: Format::text,
            summary_only: false,
            baseline_path: None,
        }
    }
}

fn default_taint_source_all() -> bool {
    true
}

impl Config {
    pub(crate) fn save(&self, path: &PathBuf) -> std::io::Result<()> {
        self.save_to(path.as_path())
    }

    /// Save configuration to `path` without CLI side effects.
    pub fn save_to(&self, path: &std::path::Path) -> std::io::Result<()> {
        let t = serde_yaml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        let mut file = std::fs::File::create(path)?;
        file.write_all(t.as_bytes())
    }

    /// Load configuration from `path`.
    ///
    /// - Missing file: returns `Ok((Config::default(), ConfigLoadInfo::CreatedDefault))` and writes defaults.
    /// - Invalid YAML: returns `Ok((Config::default(), ConfigLoadInfo::InvalidYaml(err)))` (fallback).
    /// - IO error: returns `Err`.
    ///
    /// This is GUI/embedding-friendly (no printing, no process exit).
    pub fn load_or_create(path: &std::path::Path) -> std::io::Result<(Self, ConfigLoadInfo)> {
        let default_config = Config::default();

        match std::fs::read_to_string(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Err(save_err) = default_config.save_to(path) {
                    return Err(save_err);
                }
                Ok((default_config, ConfigLoadInfo::CreatedDefault))
            }
            Err(e) => Err(e),
            Ok(s) => match serde_yaml::from_str::<Config>(&s) {
                Ok(mut c) => {
                    let default = Config::default();
                    for (code, settings) in default.rules {
                        c.rules.entry(code).or_insert(settings);
                    }
                    Ok((c, ConfigLoadInfo::Loaded))
                }
                Err(e) => Ok((default_config, ConfigLoadInfo::InvalidYaml(e.to_string()))),
            },
        }
    }

    /// Parse configuration from YAML string, filling missing rule defaults.
    pub fn from_yaml_str(s: &str) -> Result<Self, String> {
        let mut c: Config = serde_yaml::from_str(s).map_err(|e| e.to_string())?;
        let default = Config::default();
        for (code, settings) in default.rules {
            c.rules.entry(code).or_insert(settings);
        }
        Ok(c)
    }
}

/// Outcome information for [`Config::load_or_create`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLoadInfo {
    Loaded,
    CreatedDefault,
    InvalidYaml(String),
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

    #[test]
    fn deeper_pass_is_on_by_default() {
        assert!(Config::default().taint_source_all);
    }

    #[test]
    fn taint_source_all_defaults_to_true_when_absent() {
        let yaml = "enabled_rules: []\ndisable_rules: []\nrules: {}\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.taint_source_all);
    }

    #[test]
    fn taint_source_all_can_be_turned_off_from_yaml() {
        let yaml = "enabled_rules: []\ndisable_rules: []\nrules: {}\ntaint_source_all: false\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.taint_source_all);
    }

    #[test]
    fn legacy_taint_source_request_key_is_ignored() {
        // Older generated configs contain `taint_source_request: true`. The
        // deeper pass must still default on for those users after upgrading.
        let yaml = "enabled_rules: []\ndisable_rules: []\nrules: {}\ntaint_source_request: true\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.taint_source_all);
    }

    #[test]
    fn test_generation_is_opt_in() {
        assert!(Config::default().test_generation.is_none());

        let yaml = "enabled_rules: []\ndisable_rules: []\nrules: {}\ntest_generation:\n  output_dir: build/tests\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let generation = config.test_generation.unwrap();
        assert_eq!(generation.output_dir, "build/tests");
    }

    #[test]
    fn new_config_fields_have_backwards_compatible_defaults() {
        let yaml = "enabled_rules: []\ndisable_rules: []\nrules: {}\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.paths, vec!["./src".to_string()]);
        assert_eq!(config.output_format, Format::text);
        assert!(config.baseline_path.is_none());
    }

    #[test]
    fn test_generation_output_dir_has_a_default() {
        let yaml = "enabled_rules: []\ndisable_rules: []\nrules: {}\ntest_generation: {}\n";
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config.test_generation.unwrap().output_dir,
            "tests/phanalist-generated"
        );
    }
}
