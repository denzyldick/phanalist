use crate::{results::Results, rules};
use std::collections::HashMap;

use super::OutputFormatter;

use serde_json::{json, Value};

pub struct CodeClimate {}

impl OutputFormatter for CodeClimate {
    /// Produce output for CodeClimate format (that is also Gitlab-compatible)
    fn output(results: &mut Results) {
        Self::output_with_effective(results, &HashMap::new());
    }

    fn output_with_settings(results: &mut Results, settings: &HashMap<String, serde_json::Value>) {
        Self::output_with_effective(results, settings);
    }

    fn output_with_effective(results: &mut Results, settings: &HashMap<String, serde_json::Value>) {
        // CodeClimate spec:
        // https://github.com/codeclimate/platform/blob/master/spec/analyzers/SPEC.md#data-types

        let rules = rules::all_rules();
        let mut res: Vec<Value> = vec![];
        for (key, violations) in &results.files {
            for violation in violations {
                let rule_id = &violation.rule;
                let rule_markdown = match rules.get(rule_id) {
                    Some(_) => rules::render_rule_documentation(
                        &violation.rule,
                        settings.get(&violation.rule),
                    )
                    .unwrap_or_default(),
                    None => String::from("Unknown rule"),
                };

                let fingerprint = if violation.meta.fingerprint.is_empty() {
                    crate::fingerprint::compute(
                        &violation.rule,
                        key,
                        &violation.message.id,
                        &violation.line,
                    )
                } else {
                    violation.meta.fingerprint.clone()
                };

                let mut content_body = rule_markdown;
                if !violation.call_chain.is_empty() {
                    content_body.push_str(&format!(
                        "\n\nCall chain (inferred): {}",
                        violation.call_chain.join(" → ")
                    ));
                }

                let mut issue = json!({
                    "type": "issue",
                    "check_name": &violation.rule,
                    "description": violation.message.render(),
                    "content": {
                        "body": content_body
                    },
                    "categories": violation.meta.category.code_climate(),
                    "fingerprint": fingerprint,
                    "severity": violation.meta.severity.code_climate(),
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
                });
                if !violation.call_chain.is_empty() {
                    issue["call_chain"] = serde_json::json!(violation.call_chain);
                }
                res.push(issue);
            }
        }

        match serde_json::to_string(&res) {
            Ok(output) => println!("{}", output),
            Err(e) => eprintln!("Erreur de sérialisation JSON: {}", e),
        }
    }
}
