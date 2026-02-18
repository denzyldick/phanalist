use crate::{results::Results, rules};

use super::OutputFormatter;

use serde_json::{json, Value};

pub struct CodeClimate {}

impl OutputFormatter for CodeClimate {
    /// Produce output for CodeClimate format (that is also Gitlab-compatible)
    fn output(results: &mut Results) {
        // CodeClimate spec:
        // https://github.com/codeclimate/platform/blob/master/spec/analyzers/SPEC.md#data-types

        let rules = rules::all_rules();
        let mut res: Vec<Value> = vec![];
        for (key, violations) in &results.files {
            for violation in violations {
                let rule_id = &violation.rule;
                let rule_markdown = match rules.get(rule_id) {
                    Some(rule) => rule.get_detailed_explanation().unwrap_or_default(),
                    None => String::from("Unknown rule"),
                };

                res.push(json!({
                    "type": "issue",
                    "check_name": &violation.rule,
                    "description": &violation.suggestion,
                    "content": {
                        "body": &rule_markdown
                    },
                    "categories": ["Complexity"],
                    "fingerprint": "",
                    "severity": "major",
                    "location": {
                        "path": &key,
                        "positions": {
                            "begin": {
                                "line": violation.start_line,
                                "column": violation.start_column,
                            },
                            "end": {
                                "line": violation.end_line,
                                "column": violation.end_column,
                            },
                        }
                    }
                }));
            }
        }

        match serde_json::to_string(&res) {
            Ok(output) => println!("{}", output),
            Err(e) => eprintln!("Erreur de sérialisation JSON: {}", e),
        }
    }
}
