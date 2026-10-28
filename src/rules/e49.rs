use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_arguments, function_name, literal_array_len, literal_i64, literal_string,
    walk_statement_expressions,
};
use crate::rules::Rule as _;

pub(crate) static CODE: &str = "E0049";
static DESCRIPTION: &str = "Invalid builtin argument";

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
                let function_call = Call::Function(call.clone());
                let arguments = call_arguments(&function_call);
                self.check_call(file, &name, &arguments, call.span(), &mut violations);
            });
        }

        Some(violations)
    }
}

impl Rule {
    fn check_call(
        &self,
        file: &File<'_>,
        name: &str,
        arguments: &[&Expression<'_>],
        span: mago_span::Span,
        violations: &mut Vec<Violation>,
    ) {
        match name {
            "str_repeat" => {
                if let Some(count) = arguments.get(1).and_then(|argument| literal_i64(argument)) {
                    if count < 0 {
                        self.report(
                            file,
                            violations,
                            span,
                            "str_repeat",
                            format!(
                                "str_repeat() requires a non-negative repeat count, got {count}."
                            ),
                        );
                    }
                }
            }
            "array_fill" => {
                if let Some(count) = arguments.get(1).and_then(|argument| literal_i64(argument)) {
                    if count < 0 {
                        self.report(
                            file,
                            violations,
                            span,
                            "array_fill",
                            format!("array_fill() requires a non-negative count, got {count}."),
                        );
                    }
                }
            }
            "range" => {
                if let Some(step) = arguments.get(2).and_then(|argument| literal_i64(argument)) {
                    if step == 0 {
                        self.report(
                            file,
                            violations,
                            span,
                            "range",
                            String::from("range() requires a non-zero step, got 0."),
                        );
                    }
                }
            }
            "str_split" | "mb_str_split" => {
                if let Some(length) = arguments.get(1).and_then(|argument| literal_i64(argument)) {
                    if length <= 0 {
                        self.report(
                            file,
                            violations,
                            span,
                            name,
                            format!("{name}() requires a positive length, got {length}."),
                        );
                    }
                }
            }
            "explode" => {
                if let Some(delimiter) = arguments
                    .first()
                    .and_then(|argument| literal_string(argument))
                {
                    if delimiter.is_empty() {
                        self.report(
                            file,
                            violations,
                            span,
                            "explode",
                            String::from("explode() requires a non-empty delimiter."),
                        );
                    }
                }
            }
            "array_chunk" => {
                if let Some(size) = arguments.get(1).and_then(|argument| literal_i64(argument)) {
                    if size <= 0 {
                        self.report(
                            file,
                            violations,
                            span,
                            "array_chunk",
                            format!("array_chunk() requires a positive chunk size, got {size}."),
                        );
                    }
                }
            }
            "array_combine" => {
                let left = arguments
                    .first()
                    .and_then(|argument| literal_array_len(argument));
                let right = arguments
                    .get(1)
                    .and_then(|argument| literal_array_len(argument));
                if let (Some(left), Some(right)) = (left, right) {
                    if left != right {
                        self.report(
                            file,
                            violations,
                            span,
                            "array_combine",
                            format!(
                                "array_combine() requires arrays of equal length, got {left} and {right}."
                            ),
                        );
                    }
                }
            }
            "random_int" | "mt_rand" | "rand" => {
                let minimum = arguments.first().and_then(|argument| literal_i64(argument));
                let maximum = arguments.get(1).and_then(|argument| literal_i64(argument));
                if let (Some(minimum), Some(maximum)) = (minimum, maximum) {
                    if minimum > maximum {
                        self.report(
                            file,
                            violations,
                            span,
                            name,
                            format!(
                                "{name}() requires the minimum to be less than or equal to the maximum, got {minimum} > {maximum}."
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn report(
        &self,
        file: &File<'_>,
        violations: &mut Vec<Violation>,
        span: mago_span::Span,
        function: &str,
        message: String,
    ) {
        let message = Message::new(
            format!("E0049:invalid-builtin-argument:{function}"),
            "{message}",
        )
        .arg("message", message);
        violations.push(self.new_violation(file, message, span));
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_invalid_literal_arguments() {
        assert!(analyze_file_for_rule("e49/str_repeat_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e49/array_fill_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e49/range_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e49/explode_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e49/array_combine_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e49/random_int_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e49/str_split_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
        assert!(analyze_file_for_rule("e49/array_chunk_bad.php", CODE)
            .iter()
            .any(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_valid_or_nonliteral_arguments() {
        assert!(analyze_file_for_rule("e49/valid.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e49/nonliteral_safe.php", CODE).is_empty());
    }
}
