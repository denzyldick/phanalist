use mago_span::HasSpan;
use mago_syntax::cst::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{function_name, walk_statement_expressions};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0054";
static DESCRIPTION: &str = "Dynamic variable creation";

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
            walk_statement_expressions(statement, &mut |expression| match expression {
                Expression::Call(Call::Function(call)) => {
                    let Some(name) = function_name(call.function) else {
                        return;
                    };
                    if name != "extract" {
                        return;
                    }

                    let origin = taint.origin(SinkKind::Extract, call.span());
                    if self.settings.taint_mode == TaintMode::Only && origin.is_none() {
                        return;
                    }

                    let base_id = "E0054:dynamic-variable-creation:extract";
                    let template = "extract() creates local variables from runtime data. Use explicit assignments or an allowlist so unexpected keys cannot overwrite local state.";
                    let source_info = taint.sink_source_info(SinkKind::Extract, call.span(), file);
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
                }
                Expression::Variable(Variable::Indirect(variable)) => {
                    let origin = taint.origin(SinkKind::VariableVariable, variable.span());
                    if self.settings.taint_mode == TaintMode::Only && origin.is_none() {
                        return;
                    }

                    let base_id = "E0054:dynamic-variable-creation:indirect";
                    let template = "A variable variable creates a local name from runtime data. Use an explicit map or an allowlist instead.";
                    let source_info =
                        taint.sink_source_info(SinkKind::VariableVariable, variable.span(), file);
                    let message = match origin {
                        Some(_) => Message::new(base_id.to_string(), template.to_string()),
                        None => Message::new(base_id, template),
                    };
                    let mut violation = self.new_violation(file, message, variable.span());
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
                }
                Expression::Variable(Variable::Nested(variable)) => {
                    let origin = taint.origin(SinkKind::VariableVariable, variable.span());
                    if self.settings.taint_mode == TaintMode::Only && origin.is_none() {
                        return;
                    }

                    let base_id = "E0054:dynamic-variable-creation:nested";
                    let template = "A variable variable creates a local name from runtime data. Use an explicit map or an allowlist instead.";
                    let source_info =
                        taint.sink_source_info(SinkKind::VariableVariable, variable.span(), file);
                    let message = match origin {
                        Some(_) => Message::new(base_id.to_string(), template.to_string()),
                        None => Message::new(base_id, template),
                    };
                    let mut violation = self.new_violation(file, message, variable.span());
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
                }
                _ => {}
            });
        }

        Some(violations)
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_extract_and_variable_variables() {
        let violations = analyze_file_for_rule("e54/dynamic_variables_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_explicit_variables_and_compact() {
        assert!(analyze_file_for_rule("e54/dynamic_variables_safe.php", CODE).is_empty());
    }

    #[test]
    fn marks_tainted_dynamic_variables() {
        let violations = analyze_file_for_rule("e54/dynamic_variables_bad.php", CODE);
        assert!(violations
            .iter()
            .any(|violation| violation.taint_source.is_some()));
    }
}
