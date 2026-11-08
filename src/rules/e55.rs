use mago_span::HasSpan;
use mago_syntax::cst::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{function_name, literal_i64, walk_statement_expressions};
use crate::security::{FileTaint, SinkKind, TaintMode};

pub(crate) static CODE: &str = "E0055";
static DESCRIPTION: &str = "Dynamic include or require";

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
                let Expression::Construct(construct) = expression else {
                    return;
                };
                let (kind, path) = match construct {
                    Construct::Include(construct) => ("include", construct.value),
                    Construct::IncludeOnce(construct) => ("include_once", construct.value),
                    Construct::Require(construct) => ("require", construct.value),
                    Construct::RequireOnce(construct) => ("require_once", construct.value),
                    _ => return,
                };

                if !is_dynamic_path(path) {
                    return;
                }

                let origin = taint.origin(SinkKind::Include, construct.span());
                if self.settings.taint_mode == TaintMode::Only && origin.is_none() {
                    return;
                }

                let base_id = format!("E0055:dynamic-include:{kind}");
                let template = "{kind} uses a path built from runtime data. Use a literal path, a project-root constant, or choose from a fixed list of literal paths.";

                let source_info =
                    taint.sink_source_info(SinkKind::Include, construct.span(), file);
                let message = match origin {
                    Some(_) => Message::new(base_id, template),
                    None => Message::new(base_id, template).arg("kind", kind),
                };

                let mut violation = self.new_violation(file, message, construct.span());
                if let Some(info) = source_info {
                    violation.taint_source = Some(crate::results::TaintSource {
                        request_rooted: info.request_rooted,
                        source_kind: Some(format!("{:?}", info.source_kind).to_lowercase()),
                        origins: info.origins,
                        file: info.source_file,
                        line: info.source_line,
                    });
                }
                violations.push(violation);
            });
        }

        Some(violations)
    }
}

fn is_dynamic_path(expression: &Expression<'_>) -> bool {
    !is_static_path(expression)
}

fn is_static_path(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Literal(Literal::String(_)) => true,
        Expression::MagicConstant(_) => true,
        Expression::ConstantAccess(_) => true,
        Expression::Identifier(_) => true,
        Expression::Binary(binary)
            if matches!(binary.operator, BinaryOperator::StringConcat(_)) =>
        {
            is_static_path(binary.lhs) && is_static_path(binary.rhs)
        }
        Expression::Call(Call::Function(call)) => is_static_function_path(call),
        _ => false,
    }
}

fn is_static_function_path(call: &FunctionCall<'_>) -> bool {
    let Some(name) = function_name(call.function) else {
        return false;
    };

    match name.as_str() {
        "dirname" => {
            let Some(path) = argument(call, "path", 0) else {
                return false;
            };
            if !is_static_path(path) {
                return false;
            }
            argument(call, "levels", 1).is_none_or(|levels| literal_i64(levels).is_some())
        }
        "realpath" => argument(call, "path", 0).is_some_and(is_static_path),
        _ => false,
    }
}

fn argument<'a>(
    call: &'a FunctionCall<'a>,
    name: &str,
    positional_index: usize,
) -> Option<&'a Expression<'a>> {
    let mut positional = 0;
    let mut named = None;
    let mut positional_value = None;

    for argument in call.argument_list.arguments.iter() {
        match argument {
            Argument::Positional(argument) => {
                if positional == positional_index {
                    positional_value = Some(argument.value);
                }
                positional += 1;
            }
            Argument::Named(argument) => {
                let argument_name = String::from_utf8_lossy(argument.name.value);
                if argument_name.eq_ignore_ascii_case(name) {
                    named = Some(argument.value);
                }
            }
        }
    }

    named.or(positional_value)
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_runtime_include_paths() {
        let violations = analyze_file_for_rule("e55/dynamic_include_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_literal_and_project_root_paths() {
        assert!(analyze_file_for_rule("e55/dynamic_include_safe.php", CODE).is_empty());
    }

    #[test]
    fn marks_tainted_includes() {
        let violations = analyze_file_for_rule("e55/dynamic_include_bad.php", CODE);
        assert!(violations
            .iter()
            .any(|violation| violation.taint_source.is_some()));
    }
}
