use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, TaintMode};

pub(crate) static CODE: &str = "E0079";
static DESCRIPTION: &str = "Dynamic method call";

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
        taint_support::scan_callable_sinks(
            self,
            &self.settings,
            file,
            taint,
            |call| matches!(call, Call::Method(_) | Call::NullSafeMethod(_)),
            |call| match call {
                Call::Method(call) => receiver_is_dynamic(call.object),
                Call::NullSafeMethod(call) => receiver_is_dynamic(call.object),
                _ => false,
            },
            || {
                Message::new(
                    "E0079:dynamic-method-call",
                    "Dynamic method call on a user-controlled object with user-controlled arguments can lead to arbitrary code execution.",
                )
            },
        )
    }
}

fn receiver_is_dynamic(expression: &Expression<'_>) -> bool {
    taint_support::looks_dynamic(expression)
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_dynamic_method_call() {
        let violations = analyze_file_for_rule("e79/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e79/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_method_call_on_a_local_object() {
        assert!(analyze_file("<?php\n$obj = new Service();\n$obj->run();\n").is_empty());
    }

    #[test]
    fn reports_a_request_receiver() {
        let violations = analyze_file("<?php\n$obj = $_GET['obj'];\n$obj->{$method}($_GET['arg']);\n");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
