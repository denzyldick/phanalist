use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0093";
static DESCRIPTION: &str = "Template injection";

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
        let mut violations = taint_support::scan_eval(self, &self.settings, file, taint, |name| {
            Message::new(
                format!("E0093:template-injection:{name}"),
                "eval() renders a template from user-controlled input. Never render untrusted strings as templates.",
            )
        })
        .unwrap_or_default();

        let rendered = taint_support::scan_call_sinks(
            self,
            &self.settings,
            file,
            taint,
            SinkKind::Template,
            |name, is_method| is_template_sink(name, is_method),
            |name| {
                Message::new(
                    format!("E0093:template-injection:{name}"),
                    "{}() renders a template from user-controlled input. Use a template engine with auto-escaping and never render untrusted strings as templates.",
                )
                .arg("name", name)
            },
        )
        .unwrap_or_default();

        violations.extend(rendered);
        Some(violations)
    }
}

fn is_template_sink(name: &str, is_method: bool) -> bool {
    if is_method {
        matches!(name, "render" | "renderblock" | "createtemplate")
    } else {
        matches!(name, "str_replace" | "preg_replace")
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_template_injection() {
        let violations = analyze_file_for_rule("e93/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e93/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_static_template() {
        assert!(analyze_file("<?php\n$tpl = 'Hello {name}';\n").is_empty());
    }

    #[test]
    fn reports_eval_of_a_request_template() {
        let violations =
            analyze_file("<?php\n$template = $_GET['template'];\neval('echo \"' . $template . '\";');\n");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
