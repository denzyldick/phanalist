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
                    let message = match origin {
                        Some(origin) => Message::new(
                            format!("{base_id}:tainted"),
                            format!("{template} The data comes from {{source}}."),
                        )
                        .arg("source", origin)
                        .with_tainted(true),
                        None => Message::new(base_id, template),
                    };
                    violations.push(self.new_violation(file, message, call.span()));
                }
                Expression::Variable(Variable::Indirect(variable)) => {
                    let origin = taint.origin(SinkKind::VariableVariable, variable.span());
                    if self.settings.taint_mode == TaintMode::Only && origin.is_none() {
                        return;
                    }

                    let base_id = "E0054:dynamic-variable-creation:indirect";
                    let template = "A variable variable creates a local name from runtime data. Use an explicit map or an allowlist instead.";
                    let message = match origin {
                        Some(origin) => Message::new(
                            format!("{base_id}:tainted"),
                            format!("{template} The name comes from {{source}}."),
                        )
                        .arg("source", origin)
                        .with_tainted(true),
                        None => Message::new(base_id, template),
                    };
                    violations.push(self.new_violation(file, message, variable.span()));
                }
                Expression::Variable(Variable::Nested(variable)) => {
                    let origin = taint.origin(SinkKind::VariableVariable, variable.span());
                    if self.settings.taint_mode == TaintMode::Only && origin.is_none() {
                        return;
                    }

                    let base_id = "E0054:dynamic-variable-creation:nested";
                    let template = "A variable variable creates a local name from runtime data. Use an explicit map or an allowlist instead.";
                    let message = match origin {
                        Some(origin) => Message::new(
                            format!("{base_id}:tainted"),
                            format!("{template} The name comes from {{source}}."),
                        )
                        .arg("source", origin)
                        .with_tainted(true),
                        None => Message::new(base_id, template),
                    };
                    violations.push(self.new_violation(file, message, variable.span()));
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
            .any(|violation| violation.message.id.contains(":tainted")));
    }
}
