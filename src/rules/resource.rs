use serde_json::Value;

use crate::file::File;
use crate::resource::{ResourceClass, ResourceSettings};
use crate::results::Violation;

#[derive(Clone)]
pub struct Rule {
    class: ResourceClass,
    settings: ResourceSettings,
}

impl Rule {
    pub fn new(class: ResourceClass) -> Self {
        Self {
            class,
            settings: ResourceSettings::default(),
        }
    }
}

impl Default for Rule {
    fn default() -> Self {
        Self::new(ResourceClass::RuntimeGrowth)
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

    fn validate(
        &self,
        _file: &File<'_>,
        _statement: &mago_syntax::cst::Statement<'_>,
    ) -> Vec<Violation> {
        Vec::new()
    }

    fn resource_class(&self) -> Option<ResourceClass> {
        Some(self.class)
    }

    fn resource_settings(&self) -> Option<ResourceSettings> {
        Some(self.settings.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mago_allocator::prelude::LocalArena;

    use crate::analyse::Analyse;
    use crate::config::Config;
    use crate::file::File;
    use crate::rules::tests::analyze_file_for_rule;

    fn codes(path: &str, rule: &str) -> Vec<String> {
        analyze_file_for_rule(path, rule)
            .into_iter()
            .map(|violation| violation.rule)
            .collect()
    }

    fn codes_with_settings(path: &str, rule: &str, settings: serde_json::Value) -> Vec<String> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("rules")
            .join("examples")
            .join(path);
        let content = std::fs::read_to_string(&path).unwrap();
        let arena = LocalArena::new();
        let mut file = File::new(&arena, path, content);
        let mut rules = std::collections::HashMap::new();
        rules.insert(rule.to_string(), settings);
        let config = Config {
            enabled_rules: vec![rule.to_string()],
            rules,
            ..Default::default()
        };
        let analyse = Analyse::new(&config);
        analyse.begin_index();
        analyse.index_file(&file);
        analyse.finalize_index();
        analyse
            .analyse_file(&mut file, false)
            .0
            .into_iter()
            .map(|violation| violation.rule)
            .collect()
    }

    fn codes_with_indexed_files(indexed: &[&str], target: &str, rule: &str) -> Vec<String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("rules")
            .join("examples");
        let arena = LocalArena::new();
        let mut files = indexed
            .iter()
            .map(|path| {
                let path = root.join(path);
                File::new(&arena, path.clone(), std::fs::read_to_string(path).unwrap())
            })
            .collect::<Vec<_>>();
        let target_path = root.join(target);
        let mut target_file = File::new(
            &arena,
            target_path.clone(),
            std::fs::read_to_string(target_path).unwrap(),
        );
        let config = Config {
            enabled_rules: vec![rule.to_string()],
            ..Default::default()
        };
        let analyse = Analyse::new(&config);
        analyse.begin_index();
        for file in &mut files {
            analyse.index_file(file);
        }
        analyse.finalize_index();
        analyse
            .analyse_file(&mut target_file, false)
            .0
            .into_iter()
            .map(|violation| violation.rule)
            .collect()
    }

    #[test]
    fn reports_nested_input_loops() {
        assert_eq!(codes("e44/nested_bad.php", "E0044"), vec!["E0044"]);
        assert!(codes("e44/nested_safe.php", "E0044").is_empty());
        assert!(codes("e44/guarded_nested.php", "E0044").is_empty());
        assert!(codes("e44/loop_cap_safe.php", "E0044").is_empty());
        assert!(codes("e44/for_bad.php", "E0044").contains(&String::from("E0044")));
    }

    #[test]
    fn reports_copying_work_inside_a_loop() {
        assert!(codes("e44/unshift_bad.php", "E0044").contains(&String::from("E0044")));
        assert!(codes("e44/copy_safe.php", "E0044").is_empty());
        assert!(codes("e44/concat_append_safe.php", "E0044").is_empty());
        assert!(codes("e44/concat_assign_bad.php", "E0044").contains(&String::from("E0044")));
        assert!(codes("e44/costly_builtin_bad.php", "E0044").contains(&String::from("E0044")));
        assert!(codes("e44/costly_builtin_safe.php", "E0044").is_empty());
        assert!(codes("e44/costly_builtin_static_safe.php", "E0044").is_empty());
    }

    #[test]
    fn reports_retained_input_sized_arrays() {
        assert!(codes("e45/retain_bad.php", "E0045").contains(&String::from("E0045")));
        assert!(codes("e45/copy_bad.php", "E0045").contains(&String::from("E0045")));
        assert!(codes("e45/stream_safe.php", "E0045").is_empty());
        assert!(codes("e45/stream_real_safe.php", "E0045").is_empty());
        assert!(codes("e45/stream_real_bad.php", "E0045").contains(&String::from("E0045")));
        assert!(codes("e45/append_safe.php", "E0045").is_empty());
        assert!(codes("e45/local_safe.php", "E0045").is_empty());
        assert!(codes("e45/overwrite_safe.php", "E0045").is_empty());
        assert!(codes("e45/indexed_bad.php", "E0045").contains(&String::from("E0045")));
        assert!(codes("e45/validation_loop_safe.php", "E0045").is_empty());
        assert!(codes("e45/predicate_loop_safe.php", "E0045").is_empty());
        assert!(codes("e45/mutation_loop_safe.php", "E0045").is_empty());
        assert!(codes("e45/push_bad.php", "E0045").contains(&String::from("E0045")));
    }

    #[test]
    fn reports_retained_growth_without_the_input_baseline() {
        let violations = analyze_file_for_rule("e45/growth_only.php", "E0045");
        assert!(!violations.is_empty());
        let retained = violations[0]
            .message
            .args
            .iter()
            .find(|(name, _)| name == "retained")
            .map(|(_, value)| value)
            .unwrap();
        assert!(!retained.contains("bytes($items)"), "{retained}");
    }

    #[test]
    fn honors_resource_settings() {
        assert!(codes_with_settings(
            "e44/nested_bad.php",
            "E0044",
            serde_json::json!({ "runtime_threshold": "cubic" }),
        )
        .is_empty());
        assert!(codes_with_settings(
            "e46/request_loop_bad.php",
            "E0046",
            serde_json::json!({ "report_input_cardinality": false }),
        )
        .is_empty());
    }

    #[test]
    fn reports_only_the_enabled_resource_class() {
        assert!(codes("e44/nested_bad.php", "E0045").is_empty());
        assert!(codes("e45/retain_bad.php", "E0044").is_empty());
        assert!(codes("e46/request_loop_bad.php", "E0044").is_empty());
    }

    #[test]
    fn reports_unbounded_request_cardinality() {
        assert!(codes("e46/request_loop_bad.php", "E0046").contains(&String::from("E0046")));
        assert!(codes("e46/request_merge_bad.php", "E0046").contains(&String::from("E0046")));
        assert!(codes("e46/request_sink_bad.php", "E0046").contains(&String::from("E0046")));
        assert!(codes("e46/request_stream_bad.php", "E0046").contains(&String::from("E0046")));
        assert!(codes("e46/request_method_bad.php", "E0046").contains(&String::from("E0046")));
        assert!(codes("e46/non_request_method_safe.php", "E0046").is_empty());
        assert!(codes("e46/request_loop_guarded.php", "E0046").is_empty());
        assert!(codes("e46/server_metadata_safe.php", "E0046").is_empty());
        assert!(codes("e46/cli_safe.php", "E0046").is_empty());
        assert!(codes("e46/server_header_bad.php", "E0046").contains(&String::from("E0046")));
        assert!(codes_with_settings(
            "e46/cli_safe.php",
            "E0046",
            serde_json::json!({ "include_cli_input": true }),
        )
        .contains(&String::from("E0046")));
    }

    #[test]
    fn propagates_resource_summaries_across_files() {
        let codes = codes_with_indexed_files(
            &[
                "e44/summary_definition.php",
                "e46/request_helper_definition.php",
            ],
            "e44/summary_use.php",
            "E0044",
        );
        assert!(codes.contains(&String::from("E0044")), "{codes:?}");

        let codes = codes_with_indexed_files(
            &[
                "e44/summary_definition.php",
                "e46/request_helper_definition.php",
            ],
            "e46/request_helper_use.php",
            "E0046",
        );
        assert!(codes.contains(&String::from("E0046")), "{codes:?}");

        let codes = codes_with_indexed_files(
            &["e45/summary_definition.php"],
            "e45/summary_use.php",
            "E0045",
        );
        assert!(codes.contains(&String::from("E0045")), "{codes:?}");

        let codes = codes_with_indexed_files(
            &["e45/summary_definition.php"],
            "e45/summary_unused_safe.php",
            "E0045",
        );
        assert!(codes.is_empty(), "{codes:?}");

        let codes = codes_with_indexed_files(
            &["e45/summary_known_definition.php"],
            "e45/summary_known_use.php",
            "E0045",
        );
        assert!(codes.contains(&String::from("E0045")), "{codes:?}");

        let codes = codes_with_indexed_files(
            &["e44/method_definition.php"],
            "e44/method_use.php",
            "E0044",
        );
        assert!(codes.contains(&String::from("E0044")), "{codes:?}");
    }
}
