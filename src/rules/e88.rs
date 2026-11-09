use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0088";
static DESCRIPTION: &str = "Reflection-based injection";

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
            SinkKind::Reflection,
            |name, is_method| is_method && is_reflection_method(name),
            |name| {
                Message::new(
                    format!("E0088:reflection-injection:{name}"),
                    "{}() uses reflection on user-controlled input, which can expose or invoke internal members.",
                )
                .arg("name", name)
            },
        )
    }
}

fn is_reflection_method(name: &str) -> bool {
    matches!(
        name,
        "invoke" | "invokeargs" | "setaccessible" | "getvalue" | "setvalue"
    )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_reflection_injection() {
        let violations = analyze_file_for_rule("e88/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e88/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_static_member_name() {
        assert!(analyze_file("<?php\n$reflection->invoke('run');\n").is_empty());
    }

    #[test]
    fn reports_a_request_member_name() {
        let violations = analyze_file(
            "<?php\n$method = $_GET['method'];\n$reflection->invoke($method);\n",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
