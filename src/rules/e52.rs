use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{function_name, walk_expression, walk_statement_expressions};

pub(crate) static CODE: &str = "E0052";
static DESCRIPTION: &str = "Unchecked json_decode result";

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
        let mut guarded = Vec::new();

        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| match expression {
                Expression::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::NullCoalesce(_)) =>
                {
                    guarded.push(span_range(binary.lhs.span()));
                }
                Expression::Construct(Construct::Isset(construct)) => {
                    for value in construct.values.iter() {
                        guarded.push(span_range(value.span()));
                    }
                }
                Expression::Construct(Construct::Empty(construct)) => {
                    guarded.push(span_range(construct.value.span()));
                }
                _ => {}
            });
        }

        let mut violations = Vec::new();
        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| {
                let Some(call) = direct_json_decode_use(expression) else {
                    return;
                };
                if has_json_throw_on_error(call) {
                    return;
                }

                let span = call.span();
                if guarded
                    .iter()
                    .any(|(start, end)| span.start.offset >= *start && span.start.offset < *end)
                {
                    return;
                }

                let message = Message::new(
                    "E0052:unchecked-json-decode:direct-access",
                    "json_decode() can return null for invalid JSON. Check the result for null or json_last_error() before accessing it as an array or object.",
                );
                violations.push(self.new_violation(file, message, span));
            });
        }

        Some(violations)
    }
}

fn direct_json_decode_use<'a>(expression: &'a Expression<'a>) -> Option<&'a FunctionCall<'a>> {
    match expression {
        Expression::ArrayAccess(access) => json_decode_call(access.array),
        Expression::Access(Access::Property(access)) => json_decode_call(access.object),
        Expression::Call(Call::Method(call)) => json_decode_call(call.object),
        _ => None,
    }
}

fn json_decode_call<'a>(expression: &'a Expression<'a>) -> Option<&'a FunctionCall<'a>> {
    let Expression::Call(Call::Function(call)) = expression.unparenthesized() else {
        return None;
    };
    (function_name(call.function).as_deref() == Some("json_decode")).then_some(call)
}

fn has_json_throw_on_error(call: &FunctionCall<'_>) -> bool {
    for (index, argument) in call.argument_list.arguments.iter().enumerate() {
        let (name, value) = match argument {
            Argument::Positional(argument) => (None, argument.value),
            Argument::Named(argument) => (
                Some(
                    String::from_utf8_lossy(argument.name.value)
                        .trim_start_matches('\\')
                        .to_ascii_lowercase(),
                ),
                argument.value,
            ),
        };

        let is_flags = name.as_deref() == Some("flags") || (name.is_none() && index == 3);
        if is_flags && contains_throw_constant(value) {
            return true;
        }
    }

    false
}

fn contains_throw_constant(expression: &Expression<'_>) -> bool {
    let mut found = false;
    walk_expression(expression, &mut |expression| {
        let Expression::ConstantAccess(constant) = expression else {
            return;
        };
        if String::from_utf8_lossy(constant.name.value()).trim_start_matches('\\')
            == "JSON_THROW_ON_ERROR"
        {
            found = true;
        }
    });
    found
}

fn span_range(span: Span) -> (u32, u32) {
    (span.start.offset, span.end.offset)
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_direct_access_to_json_decode_results() {
        let violations = analyze_file_for_rule("e52/unchecked_json_decode_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_checked_or_guarded_results() {
        assert!(analyze_file_for_rule("e52/unchecked_json_decode_safe.php", CODE).is_empty());
    }
}
