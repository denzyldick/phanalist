use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{call_arguments, function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0059";
static DESCRIPTION: &str = "Dynamic eval or assert";

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

                let call = Call::Function(call.clone());
                let arguments = call_arguments(&call);

                let message = match name.as_str() {
                    "eval" => Some(Message::new(
                        "E0059:dynamic-eval:call",
                        "eval() executes code built at runtime. Replace it with a fixed parser, dispatch table, or explicit callback.",
                    )),
                    "assert"
                        if arguments
                            .first()
                            .is_some_and(|argument| is_code_string(argument)) =>
                    {
                        Some(Message::new(
                            "E0059:dynamic-eval:assert-string",
                            "assert() is given a string of code. Use a boolean expression or a dedicated validation library instead.",
                        ))
                    }
                    _ => None,
                };

                if let Some(message) = message {
                    violations.push(self.new_violation(file, message, call.span()));
                }
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
}
