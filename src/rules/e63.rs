use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0063";
static DESCRIPTION: &str = "LDAP injection";

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
            SinkKind::Ldap,
            |name, is_method| !is_method && is_ldap_function(name),
            |name| {
                Message::new(
                    format!("E0063:ldap-injection:{name}"),
                    "{}() builds an LDAP filter from user-controlled input. Use ldap_escape() with LDAP_ESCAPE_FILTER.",
                )
                .arg("name", name)
            },
        )
    }
}

fn is_ldap_function(name: &str) -> bool {
    matches!(name, "ldap_search" | "ldap_list" | "ldap_read")
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_ldap_injection() {
        let violations = analyze_file_for_rule("e63/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e63/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_filter_built_from_literals() {
        assert!(analyze_file(
            "<?php\n$filter = '(cn=static)';\nldap_search($conn, 'ou=users', $filter);\n"
        )
        .is_empty());
    }

    #[test]
    fn reports_an_interpolated_filter() {
        let violations = analyze_file(
            "<?php\n$cn = $_GET['cn'];\nldap_search($conn, 'ou=users', '(cn=' . $cn . ')');\n",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
