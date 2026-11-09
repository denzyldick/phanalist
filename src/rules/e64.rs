use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0064";
static DESCRIPTION: &str = "XPath injection";

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
            SinkKind::Xml,
            |name, is_method| is_xpath_function(name, is_method),
            |name| {
                Message::new(
                    format!("E0064:xpath-injection:{name}"),
                    "{}() builds an XPath query from user-controlled input. Use parameterized XPath or escape user input.",
                )
                .arg("name", name)
            },
        )
    }
}

fn is_xpath_function(name: &str, is_method: bool) -> bool {
    if is_method {
        matches!(name, "load" | "loadxml")
    } else {
        matches!(name, "simplexml_load_string")
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_xpath_injection() {
        let violations = analyze_file_for_rule("e64/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e64/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_static_xml() {
        assert!(analyze_file(
            "<?php\n$xml = simplexml_load_string('<root><item>value</item></root>');\n"
        )
        .is_empty());
    }

    #[test]
    fn reports_request_xml() {
        let violations = analyze_file("<?php\nsimplexml_load_string($_POST['xml']);\n");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
