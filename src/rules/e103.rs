use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0103";
static DESCRIPTION: &str = "Server-side request forgery";

#[derive(Default)]
pub struct Rule {
    settings: Settings,
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        false
    }

    fn validate(&self, _file: &File<'_>, _statement: &Statement<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn taint_mode(&self) -> Option<TaintMode> {
        Some(self.settings.taint_mode)
    }

    fn settings_json(&self) -> Option<Value> {
        serde_json::to_value(&self.settings).ok()
    }

    fn set_config(&mut self, json: &Value) {
        if let Ok(settings) = serde_json::from_value::<Settings>(json.clone()) {
            self.settings = settings;
        }
    }

    fn validate_file_with_taint(
        &self,
        file: &File<'_>,
        taint: &FileTaint,
    ) -> Option<Vec<Violation>> {
        taint_support::scan_call_sinks(
            self,
            &self.settings,
            file,
            taint,
            SinkKind::Ssrf,
            |name, is_method| !is_method && is_ssrf_function(name),
            |name| {
                Message::new(
                    format!("E0103:ssrf:{name}"),
                    "{}() makes a network request to a user-controlled URL. Validate the URL against an allowlist to prevent SSRF.",
                )
                .arg("name", name)
            },
        )
    }
}

fn is_ssrf_function(name: &str) -> bool {
    matches!(
        name,
        "file_get_contents" | "fopen" | "curl_init" | "curl_exec" | "fsockopen"
    )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_ssrf() {
        let violations = analyze_file_for_rule("e103/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e103/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_static_url() {
        assert!(analyze_file(
            "<?php\n$url = 'https://api.example.com/data';\nfile_get_contents($url);\n"
        )
        .is_empty());
    }

    #[test]
    fn reports_request_data_reaching_a_url_helper() {
        let violations =
            analyze_file("<?php\n$url = $_GET['url'];\ncurl_init($url);\n");
        assert_eq!(violations.len(), 1);
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
