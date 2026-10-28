use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{call_arguments, function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0051";
static DESCRIPTION: &str = "Count of a non-countable value";

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
                if !matches!(name.as_str(), "count" | "sizeof") {
                    return;
                }

                let call = Call::Function(call.clone());
                let arguments = call_arguments(&call);
                let Some(argument) = arguments.first() else {
                    return;
                };
                let Some(kind) = non_countable_literal(argument) else {
                    return;
                };

                let message = Message::new(
                    format!("E0051:count-non-countable:{name}"),
                    "{name}() requires an array or Countable value, but its argument is always {kind}.",
                )
                .arg("name", name)
                .arg("kind", kind);

                violations.push(self.new_violation(file, message, call.span()));
            });
        }

        Some(violations)
    }
}

fn non_countable_literal(expression: &Expression<'_>) -> Option<&'static str> {
    match expression.unparenthesized() {
        Expression::Literal(Literal::String(_)) => Some("a string"),
        Expression::Literal(Literal::Integer(_)) => Some("an integer"),
        Expression::Literal(Literal::Float(_)) => Some("a float"),
        Expression::Literal(Literal::True(_)) | Expression::Literal(Literal::False(_)) => {
            Some("a boolean")
        }
        Expression::Literal(Literal::Null(_)) => Some("null"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_count_of_literal_scalars() {
        let violations = analyze_file_for_rule("e51/non_countable_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_arrays_and_unknown_values() {
        assert!(analyze_file_for_rule("e51/countable_safe.php", CODE).is_empty());
    }
}
