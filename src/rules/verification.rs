use serde_json::Value;

use crate::config::Config;
use crate::file::File;
use crate::results::Violation;
use crate::verification::{FindingClass, Settings};

#[derive(Clone)]
pub struct Rule {
    class: FindingClass,
    settings: Settings,
}

impl Rule {
    pub fn new(class: FindingClass) -> Self {
        Self {
            class,
            settings: Settings::default(),
        }
    }
}

impl Default for Rule {
    fn default() -> Self {
        Self::new(FindingClass::Division)
    }
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(self.class.code())
    }

    fn description(&self) -> String {
        String::from(self.class.description())
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        false
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        }
    }

    fn read_config(&mut self, config: &Config) {
        let code = self.get_code();
        if let Some(rule_config) = config.rules.get(&code) {
            self.set_config(rule_config);
            return;
        }

        if code != "E0031" {
            if let Some(legacy_config) = config.rules.get("E0031") {
                self.set_config(legacy_config);
            }
        }
    }

    fn validate(
        &self,
        _file: &File<'_>,
        _statement: &mago_syntax::cst::Statement<'_>,
    ) -> Vec<Violation> {
        Vec::new()
    }

    fn validate_file(&self, _file: &File<'_>) -> Option<Vec<Violation>> {
        None
    }

    fn verification_class(&self) -> Option<FindingClass> {
        Some(self.class)
    }

    fn verification_settings(&self) -> Option<Settings> {
        Some(self.settings.clone())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::rules::Rule as _;

    fn configured_rule(code: FindingClass, rules: serde_json::Value) -> Rule {
        let config: Config = serde_json::from_value(json!({
            "enabled_rules": [],
            "disable_rules": [],
            "rules": rules,
        }))
        .unwrap();
        let mut rule = Rule::new(code);
        rule.read_config(&config);
        rule
    }

    #[test]
    fn legacy_e0031_settings_apply_when_a_class_has_no_block() {
        let rule = configured_rule(
            FindingClass::ArrayIndex,
            json!({
                "E0031": { "timeout_ms": 4321, "unroll_depth": 3 }
            }),
        );

        assert_eq!(rule.settings.timeout_ms, 4321);
        assert_eq!(rule.settings.unroll_depth, 3);
    }

    #[test]
    fn class_specific_settings_take_precedence_over_legacy_e0031() {
        let rule = configured_rule(
            FindingClass::ArrayIndex,
            json!({
                "E0031": { "timeout_ms": 4321 },
                "E0036": { "timeout_ms": 1234 }
            }),
        );

        assert_eq!(rule.settings.timeout_ms, 1234);
    }
}
