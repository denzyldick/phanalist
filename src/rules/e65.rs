use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0065";
static DESCRIPTION: &str = "NoSQL injection";

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
            SinkKind::Nosql,
            |name, is_method| is_nosql_function(name, is_method),
            |name| {
                Message::new(
                    format!("E0065:nosql-injection:{name}"),
                    "{}() builds a NoSQL query from user-controlled input. Use parameterized queries or strict type checking.",
                )
                .arg("name", name)
            },
        )
    }
}

fn is_nosql_function(name: &str, is_method: bool) -> bool {
    if is_method {
        matches!(
            name,
            "insertone"
                | "insertmany"
                | "updateone"
                | "updatemany"
                | "replaceone"
                | "deleteone"
                | "deletemany"
                | "findone"
                | "find"
                | "aggregate"
                | "countdocuments"
                | "distinct"
                | "createclient"
        )
    } else {
        matches!(name, "mongo_insert" | "mongo_update" | "mongo_find")
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_nosql_injection() {
        let violations = analyze_file_for_rule("e65/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e65/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_literal_filter() {
        assert!(analyze_file(
            "<?php\n$collection->insertOne(['name' => 'alice', 'role' => 'user']);\n"
        )
        .is_empty());
    }

    #[test]
    fn reports_a_request_filter() {
        let violations =
            analyze_file("<?php\n$filter = $_GET['filter'];\n$collection->insertOne($filter);\n");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
