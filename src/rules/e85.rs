use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::walk_statement_expressions;

pub(crate) static CODE: &str = "E0085";
static DESCRIPTION: &str = "Inline PHP execution short tags";

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
            for nested in self.flatten_statements_to_validate(statement) {
                if let Statement::EchoTag(_) = nested {
                    let message = Message::new(
                        "E0085:short-echo-tag",
                        "Short echo tag <?= ?> is used. Use full <?php echo ?> tags for clarity and portability.",
                    );
                    violations.push(self.new_violation(file, message, nested.span()));
                }
            }
        }

        Some(violations)
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_short_echo_tag() {
        let violations = analyze_file_for_rule("e85/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e85/safe.php", CODE).is_empty());
    }
}
