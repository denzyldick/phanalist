use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::walk_statement_expressions;

pub(crate) static CODE: &str = "E0055";
static DESCRIPTION: &str = "Dynamic include or require";

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

                let message = Message::new(
                    format!("E0055:dynamic-include:{kind}"),
                    "{kind} uses a path built from runtime data. Use a fixed path, a project-root constant, or an explicit allowlist before including the file.",
                )
                .arg("kind", kind);

                violations.push(self.new_violation(file, message, construct.span()));
            });
        }

        Some(violations)
    }
}

fn is_dynamic_path(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Literal(Literal::String(_)) => false,
        Expression::MagicConstant(_) => false,
        Expression::ConstantAccess(_) => false,
        Expression::Identifier(_) => false,
        Expression::Binary(binary)
            if matches!(binary.operator, BinaryOperator::StringConcat(_)) =>
        {
            is_dynamic_path(binary.lhs) || is_dynamic_path(binary.rhs)
        }
        _ => true,
    }
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
}
