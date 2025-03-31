use crate::results::Results;

use super::OutputFormatter;

use serde_json::{json, Value};

pub struct GitlabFormat {}

impl OutputFormatter for GitlabFormat {
    fn output(results: &mut Results) {
        // todo!("Implement gitlab code quality report format https://docs.gitlab.com/ci/testing/code_quality/#code-quality-report-format");

        let mut res: Vec<Value> = vec![];
        for (key, violations) in &results.files {
            for violation in violations {
                res.push(json!({
                    "description": &violation.suggestion,
                    "check_name": &violation.rule,
                    "fingerprint": "",
                    "severity": "major",
                    "location": {
                        "path": &key,
                        "lines": {
                            "begin": &violation.span.line
                        }
                    }
                }));
            }
        }

        let output = serde_json::to_string(&res).unwrap();
        println!("{}", output);
    }
}
