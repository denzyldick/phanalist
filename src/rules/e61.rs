use mago_span::HasSpan;
use mago_syntax::cst::*;
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_arguments, call_name, call_span, is_method_call, walk_statement_expressions,
};
use crate::rules::Rule as _;
use crate::rules::taint_support::{self, Settings};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0061";
static DESCRIPTION: &str = "Cross-Site Scripting (XSS)";

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
            for nested in self.flatten_statements_to_validate(statement) {
                match nested {
                    Statement::Echo(echo) => {
                        for value in echo.values.iter() {
                            self.check_output(
                                file,
                                taint,
                                value,
                                value.span(),
                                "E0061:xss:echo",
                                "echo() outputs user-controlled input without HTML escaping. Use htmlspecialchars() or a template engine with auto-escaping.",
                                &mut violations,
                            );
                        }
                    }
                    Statement::EchoTag(echo_tag) => {
                        for value in echo_tag.values.iter() {
                            self.check_output(
                                file,
                                taint,
                                value,
                                value.span(),
                                "E0061:xss:short-echo-tag",
                                "Short echo tag outputs user-controlled input without HTML escaping. Use htmlspecialchars() or a template engine with auto-escaping.",
                                &mut violations,
                            );
                        }
                    }
                    _ => {}
                }
            }

            walk_statement_expressions(statement, &mut |expression| {
                match expression {
                    Expression::Construct(Construct::Print(print_)) => {
                        self.check_output(
                            file,
                            taint,
                            &print_.value,
                            print_.value.span(),
                            "E0061:xss:print",
                            "print() outputs user-controlled input without HTML escaping. Use htmlspecialchars() or a template engine with auto-escaping.",
                            &mut violations,
                        );
                    }
                    Expression::Call(call) => {
                        let Some(name) = call_name(call) else {
                            return;
                        };
                        if is_method_call(call) || !is_output_function(&name) {
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
                            SinkKind::Html,
                            span,
                            legacy,
                        ) {
                            return;
                        }
                        let message = Message::new(
                            format!("E0061:xss:{name}"),
                            "{}() outputs user-controlled input without HTML escaping. Use htmlspecialchars() or a template engine with auto-escaping.",
                        )
                        .arg("name", name);
                        let mut violation = self.new_violation(file, message, span);
                        taint_support::annotate(
                            &self.settings,
                            &mut violation,
                            taint,
                            SinkKind::Html,
                            span,
                            file,
                        );
                        violations.push(violation);
                    }
                    _ => {}
                }
            });
        }

        Some(violations)
    }
}

impl Rule {
    #[allow(clippy::too_many_arguments)]
    fn check_output(
        &self,
        file: &File<'_>,
        taint: &FileTaint,
        value: &Expression<'_>,
        span: mago_span::Span,
        id: &str,
        template: &str,
        violations: &mut Vec<Violation>,
    ) {
        let legacy = taint_support::looks_dynamic(value);
        if !taint_support::should_report(&self.settings, taint, SinkKind::Html, span, legacy) {
            return;
        }

        let message = Message::new(id.to_string(), template.to_string());
        let mut violation = self.new_violation(file, message, span);
        taint_support::annotate(
            &self.settings,
            &mut violation,
            taint,
            SinkKind::Html,
            span,
            file,
        );
        violations.push(violation);
    }
}

fn is_output_function(name: &str) -> bool {
    matches!(name, "printf" | "vprintf")
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_xss() {
        let violations = analyze_file_for_rule("e61/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e61/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_output_of_a_local_variable() {
        assert!(analyze_file("<?php\n$title = 'Hello';\necho $title;\n").is_empty());
    }

    #[test]
    fn reports_interpolated_request_output() {
        let violations = analyze_file("<?php\n$name = $_GET['name'];\necho \"Hello $name\";\n");
        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].taint_source.as_ref().unwrap().origins,
            vec!["$_GET['name']".to_string()]
        );
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
