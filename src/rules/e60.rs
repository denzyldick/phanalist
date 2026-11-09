use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_arguments, call_name, call_span, is_method_call, walk_statement_expressions,
};
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0060";
static DESCRIPTION: &str = "SQL injection";

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
        let program = file.ast?;
        let mut violations = Vec::new();

        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| {
                let Expression::Call(call) = expression else {
                    return;
                };
                let Some(name) = call_name(call) else {
                    return;
                };
                if !is_sql_call(&name, is_method_call(call)) {
                    return;
                }

                let arguments = call_arguments(call);
                if arguments.is_empty() {
                    return;
                }

                let span = call_span(call);
                let legacy = arguments
                    .iter()
                    .any(|argument| taint_support::looks_dynamic(argument));
                if !taint_support::should_report(
                    &self.settings,
                    taint,
                    SinkKind::Sql,
                    span,
                    legacy,
                ) {
                    return;
                }

                let message = Message::new(
                    format!("E0060:sql-injection:{name}"),
                    "{}() executes a SQL query built from user-controlled input. Use prepared statements with bound parameters instead.",
                )
                .arg("name", name);

                let mut violation = self.new_violation(file, message, span);
                taint_support::annotate(
                    &self.settings,
                    &mut violation,
                    taint,
                    SinkKind::Sql,
                    span,
                    file,
                );
                violations.push(violation);
            });
        }

        Some(violations)
    }
}

fn is_sql_call(name: &str, is_method: bool) -> bool {
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
    fn reports_sql_injection() {
        let violations = analyze_file_for_rule("e60/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e60/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_a_query_built_from_a_local_literal() {
        assert!(analyze_file("<?php\n$sql = \"SELECT * FROM users\";\nmysqli_query($db, $sql);\n")
            .is_empty());
    }

    #[test]
    fn reports_a_query_built_from_an_interpolated_request_value() {
        let violations = analyze_file(
            "<?php\n$id = $_GET['id'];\nmysqli_query($db, \"SELECT * FROM users WHERE id = $id\");\n",
        );
        assert_eq!(violations.len(), 1);
        assert!(violations[0].taint_source.is_some());
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
