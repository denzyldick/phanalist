use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, TaintMode};

pub(crate) static CODE: &str = "E0123";
static DESCRIPTION: &str = "XSS via echo";

#[derive(Default)]
pub struct Rule {}

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

    fn taint_mode(&self) -> Option<crate::security::TaintMode> {
        Some(TaintMode::Only)
    }

    fn settings_json(&self) -> Option<serde_json::Value> {
        None
    }

    fn set_config(&mut self, _json: &serde_json::Value) {}

    fn validate_file_with_taint(
        &self,
        file: &File<'_>,
        taint: &crate::security::FileTaint,
    ) -> Option<Vec<Violation>> {
        taint_support::scan_output(self, &Settings::default(), file, taint, |_| {
            Message::new(
                "E0123:xss-echo",
                "echo() outputs user-controlled input without HTML escaping, leading to cross-site scripting.",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_xss_echo() {
        let violations = analyze_file_for_rule("e123/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e123/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_literal_echo() {
        assert!(analyze_file("<?php\necho 'Hello';\\n").is_empty());
    }

    #[test]
    fn reports_request_echo() {
        let violations = analyze_file("<?php\necho $_GET['name'];\\n");
        assert_eq!(violations.len(), 1);
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}