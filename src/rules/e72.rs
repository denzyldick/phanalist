use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0072";
static DESCRIPTION: &str = "Path traversal";

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
            SinkKind::FilePath,
            |name, is_method| !is_method && is_file_function(name),
            |name| {
                Message::new(
                    format!("E0072:path-traversal:{name}"),
                    "{}() opens a file from user-controlled input. Validate the path to prevent directory traversal.",
                )
                .arg("name", name)
            },
        )
    }
}

fn is_file_function(name: &str) -> bool {
    matches!(
        name,
        "file_get_contents"
            | "file"
            | "readfile"
            | "fopen"
            | "file_put_contents"
            | "unlink"
            | "file_exists"
            | "is_file"
            | "is_dir"
            | "scandir"
            | "opendir"
    )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_path_traversal() {
        let violations = analyze_file_for_rule("e72/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e72/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_local_path() {
        assert!(analyze_file("<?php\n$path = __DIR__ . '/config.txt';\nreadfile($path);\n")
            .is_empty());
    }

    #[test]
    fn reports_request_data_reaching_a_file_helper() {
        let violations = analyze_file("<?php\nreadfile($_POST['path']);\n");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
