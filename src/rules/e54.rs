use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{function_name, walk_statement_expressions};

pub(crate) static CODE: &str = "E0054";
static DESCRIPTION: &str = "Dynamic variable creation";

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
            walk_statement_expressions(statement, &mut |expression| match expression {
                Expression::Call(Call::Function(call)) => {
                    let Some(name) = function_name(call.function) else {
                        return;
                    };
                    if name != "extract" {
                        return;
                    }

                    let message = Message::new(
                        "E0054:dynamic-variable-creation:extract",
                        "extract() creates local variables from runtime data. Use explicit assignments or an allowlist so unexpected keys cannot overwrite local state.",
                    );
                    violations.push(self.new_violation(file, message, call.span()));
                }
                Expression::Variable(Variable::Indirect(variable)) => {
                    let message = Message::new(
                        "E0054:dynamic-variable-creation:indirect",
                        "A variable variable creates a local name from runtime data. Use an explicit map or an allowlist instead.",
                    );
                    violations.push(self.new_violation(file, message, variable.span()));
                }
                Expression::Variable(Variable::Nested(variable)) => {
                    let message = Message::new(
                        "E0054:dynamic-variable-creation:nested",
                        "A variable variable creates a local name from runtime data. Use an explicit map or an allowlist instead.",
                    );
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
}
