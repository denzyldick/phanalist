use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{function_name, literal_string, walk_statement_expressions};

pub(crate) static CODE: &str = "E0053";
static DESCRIPTION: &str = "Unserialize without allowed_classes";

#[derive(Default)]
pub struct Rule {}

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

    fn validate_file(&self, file: &File<'_>) -> Option<Vec<Violation>> {
        let program = file.ast?;
        let mut violations = Vec::new();

        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| {
                let Expression::Call(Call::Function(call)) = expression else {
                    return;
                };
                let Some(name) = function_name(call.function) else {
                    return;
                };
                if name != "unserialize" {
                    return;
                }

                let Some(data) = argument(call, "data", 0) else {
                    return;
                };
                if is_safe_input(data) {
                    return;
                }

                let reason = match argument(call, "options", 1) {
                    None => "missing",
                    Some(options) => match allowed_classes_option(options) {
                        Some(OptionValue::Missing) => "missing",
                        Some(OptionValue::Enabled) => "enabled",
                        Some(OptionValue::Restricted) | None => return,
                    },
                };

                let message = match reason {
                    "enabled" => Message::new(
                        "E0053:unserialize-allowed-classes:enabled",
                        "unserialize() explicitly enables every class. Set allowed_classes to false or to a list of trusted classes.",
                    ),
                    _ => Message::new(
                        "E0053:unserialize-allowed-classes:missing",
                        "unserialize() is called without an allowed_classes option, so every class can be instantiated. Set allowed_classes to false or to a list of trusted classes.",
                    ),
                };

                violations.push(self.new_violation(file, message, call.span()));
            });
        }

        Some(violations)
    }
}

enum OptionValue {
    Missing,
    Enabled,
    Restricted,
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

fn allowed_classes_option(expression: &Expression<'_>) -> Option<OptionValue> {
    let elements = match expression.unparenthesized() {
        Expression::Array(array) => &array.elements,
        Expression::LegacyArray(array) => &array.elements,
        _ => return None,
    };

    for element in elements.iter() {
        let ArrayElement::KeyValue(element) = element else {
            continue;
        };
        if literal_string(element.key).as_deref() != Some("allowed_classes") {
            continue;
        }

        return Some(if is_literal_true(element.value) {
            OptionValue::Enabled
        } else {
            OptionValue::Restricted
        });
    }

    Some(OptionValue::Missing)
}

fn is_safe_input(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Literal(Literal::String(_)) => true,
        Expression::Call(Call::Function(call)) => {
            function_name(call.function).as_deref() == Some("serialize")
        }
        _ => false,
    }
}

fn is_literal_true(expression: &Expression<'_>) -> bool {
    matches!(
        expression.unparenthesized(),
        Expression::Literal(Literal::True(_))
    )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_missing_or_enabled_allowed_classes() {
        let violations = analyze_file_for_rule("e53/unserialize_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_restricted_or_dynamic_options() {
        assert!(analyze_file_for_rule("e53/unserialize_safe.php", CODE).is_empty());
    }
}
