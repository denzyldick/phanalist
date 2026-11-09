use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::walk_statement_expressions;
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0105";
static DESCRIPTION: &str = "Local file inclusion";

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

    fn settings_json(&self) -> Option<serde_json::Value> {
        serde_json::to_value(&self.settings).ok()
    }

    fn set_config(&mut self, json: &serde_json::Value) {
        if let Ok(settings) = serde_json::from_value::<Settings>(json.clone()) {
            self.settings = settings;
        }
    }

    fn validate_file_with_taint(
        &self,
        file: &File<'_>,
        taint: &FileTaint,
    ) -> Option<Vec<Violation>> {
        taint_support::scan_includes(self, &self.settings, file, taint, |name| {
            Message::new(
                format!("E0105:local-file-inclusion:{name}"),
                "include/require includes a local file from user-controlled input. Validate the path against an allowlist.",
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_lfi() {
        let violations = analyze_file_for_rule("e105/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e105/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_literal_path() {
        assert!(analyze_file(
            "<?php\ninclude __DIR__ . '/config.php';\n"
        )
        .is_empty());
    }

    #[test]
    fn reports_request_path() {
        let violations = analyze_file("<?php\ninclude $_GET['file'];\\n");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}