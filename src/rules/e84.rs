use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0084";
static DESCRIPTION: &str = "SQL query string concatenation";

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
            SinkKind::Sql,
            |name, is_method| is_query_function(name, is_method),
            |name| {
                Message::new(
                    format!("E0084:sql-concat:{name}"),
                    "{}() builds a query string from user-controlled input. Use prepared statements with bound parameters.",
                )
                .arg("name", name)
            },
        )
    }
}

fn is_query_function(name: &str, is_method: bool) -> bool {
    if is_method {
        matches!(name, "query" | "exec" | "real_query" | "multi_query")
    } else {
        matches!(
            name,
            "mysqli_query"
                | "mysqli_real_query"
                | "mysqli_multi_query"
                | "mysql_query"
                | "pg_query"
                | "pg_exec"
                | "pg_send_query"
                | "sqlite_query"
                | "sqlite_exec"
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_sql_concatenation() {
        let violations = analyze_file_for_rule("e84/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e84/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_literal_query() {
        assert!(
            analyze_file("<?php\nmysqli_query($conn, 'SELECT * FROM users');\n").is_empty()
        );
    }

    #[test]
    fn reports_a_request_query() {
        let violations = analyze_file(
            "<?php\n$id = $_GET['id'];\nmysqli_query($conn, 'SELECT * FROM users WHERE id = ' . $id);\n",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
