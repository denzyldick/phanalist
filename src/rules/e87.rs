use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, TaintMode};

pub(crate) static CODE: &str = "E0087";
static DESCRIPTION: &str = "Command execution with backticks";

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
        taint_support::scan_backticks(self, &self.settings, file, taint, || {
            Message::new(
                "E0087:backtick-command",
                "Backtick command execution with interpolated input can lead to command injection. Use escapeshellarg() on user input.",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_backtick_command() {
        let violations = analyze_file_for_rule("e87/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e87/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_literal_backtick_command() {
        assert!(analyze_file("<?php\n$output = `ls -la`;\n").is_empty());
    }

    #[test]
    fn reports_request_interpolation() {
        let violations = analyze_file("<?php\n$dir = $_GET['dir'];\n$output = `ls $dir`;\n");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
