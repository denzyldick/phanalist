use mago_span::HasSpan;
use mago_syntax::cst::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{call_arguments, function_name, walk_statement_expressions};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0059";
static DESCRIPTION: &str = "Dynamic eval or assert";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub taint_mode: TaintMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            taint_mode: TaintMode::Annotate,
        }
    }
}

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
                if let Expression::Construct(Construct::Eval(construct)) = expression {
                    let origin = taint.origin(SinkKind::Eval, construct.span());
                    if self.settings.taint_mode == TaintMode::Only && origin.is_none() {
                        return;
                    }

                    let base_id = "E0059:dynamic-eval:construct";
                    let template = "eval() executes code built at runtime. Replace it with a fixed parser, dispatch table, or explicit callback.";
                    let source_info = taint.sink_source_info(SinkKind::Eval, construct.span(), file);
                    let message = match origin {
                        Some(_) => Message::new(base_id.to_string(), template.to_string()),
                        None => Message::new(base_id, template),
                    };
                    let mut violation = self.new_violation(file, message, construct.span());
                    if let Some(info) = source_info {
                        violation.taint_source = Some(crate::results::TaintSource {
                            request_rooted: info.request_rooted,
                            source_kind: Some(format!("{:?}", info.source_kind).to_lowercase()),
                            origins: info.origins,
                            file: info.source_file,
                            line: info.source_line,
                            enclosing_function: info.enclosing_function,
                        });
                    }
                    violations.push(violation);
                    return;
                }

                let Expression::Call(Call::Function(call)) = expression else {
                    return;
                };
                let Some(name) = function_name(call.function) else {
                    return;
                };
                if name != "assert" {
                    return;
                }

                let call = Call::Function(call.clone());
                let arguments = call_arguments(&call);
                if !arguments
                    .first()
                    .is_some_and(|argument| is_code_string(argument))
                {
                    return;
                }

                let origin = taint.origin(SinkKind::AssertString, call.span());
                if self.settings.taint_mode == TaintMode::Only && origin.is_none() {
                    return;
                }

                let base_id = "E0059:dynamic-eval:assert-string";
                let template = "assert() is given a string of code. Use a boolean expression or a dedicated validation library instead.";
                let source_info = taint.sink_source_info(SinkKind::AssertString, call.span(), file);
                let message = match origin {
                    Some(_) => Message::new(base_id.to_string(), template.to_string()),
                    None => Message::new(base_id, template),
                };
                let mut violation = self.new_violation(file, message, call.span());
                if let Some(info) = source_info {
                    violation.taint_source = Some(crate::results::TaintSource {
                        request_rooted: info.request_rooted,
                        source_kind: Some(format!("{:?}", info.source_kind).to_lowercase()),
                        origins: info.origins,
                        file: info.source_file,
                        line: info.source_line,
                        enclosing_function: info.enclosing_function,
                    });
                }
                violations.push(violation);
            });
        }

        Some(violations)
    }
}

fn is_code_string(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Literal(Literal::String(_)) => true,
        Expression::Binary(binary)
            if matches!(binary.operator, BinaryOperator::StringConcat(_)) =>
        {
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_eval_and_string_assertions() {
        let violations = analyze_file_for_rule("e59/dynamic_eval_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_boolean_assertions_and_callbacks() {
        assert!(analyze_file_for_rule("e59/dynamic_eval_safe.php", CODE).is_empty());
    }

    #[test]
    fn marks_tainted_eval_flows() {
        let violations = analyze_file_for_rule("e59/dynamic_eval_bad.php", CODE);
        assert!(violations
            .iter()
            .any(|violation| violation.taint_source.is_some()));
    }
}
