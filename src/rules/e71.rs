use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0071";
static DESCRIPTION: &str = "Unsafe file upload";

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
            |name, is_method| !is_method && is_upload_function(name),
            |name| {
                Message::new(
                    format!("E0071:unsafe-file-upload:{name}"),
                    "{}() writes an uploaded file to a user-controlled destination. Validate the target path to prevent path traversal.",
                )
                .arg("name", name)
            },
        )
    }
}

fn is_upload_function(name: &str) -> bool {
    matches!(name, "move_uploaded_file" | "file_put_contents")
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_unsafe_upload() {
        let violations = analyze_file_for_rule("e71/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e71/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_fixed_destination() {
        assert!(analyze_file(
            "<?php\nmove_uploaded_file($_FILES['file']['tmp_name'], '/var/www/uploads/file.txt');\n"
        )
        .is_empty());
    }

    #[test]
    fn reports_a_request_destination() {
        let violations = analyze_file(
            "<?php\n$dest = $_GET['dest'];\nmove_uploaded_file($_FILES['file']['tmp_name'], $dest);\n",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
