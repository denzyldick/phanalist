use mago_span::HasSpan;
use mago_syntax::cst::*;
use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::walk_statement_expressions;

pub(crate) static CODE: &str = "E0105";
static DESCRIPTION: &str = "Local file inclusion";

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
                if let Expression::Construct(construct) = expression {
                    let value = match construct {
                        Construct::Include(inc) => &inc.value,
                        Construct::IncludeOnce(inc) => &inc.value,
                        Construct::Require(req) => &req.value,
                        Construct::RequireOnce(req) => &req.value,
                        _ => return,
                    };
                    if has_user_input(value) {
                        let message = Message::new(
                            format!("E0105:local-file-inclusion:{}", construct_name(construct)),
                            "include/require includes a local file from user-controlled input. Validate the path against an allowlist.",
                        );
                        violations.push(self.new_violation(file, message, value.span()));
                    }
                }
            });
        }

        Some(violations)
    }
}

fn construct_name(construct: &Construct<'_>) -> &'static str {
    match construct {
        Construct::Include(_) => "include",
        Construct::IncludeOnce(_) => "include_once",
        Construct::Require(_) => "require",
        Construct::RequireOnce(_) => "require_once",
        _ => "include",
    }
}

fn has_user_input(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::ArrayAccess(access) => {
            if let Expression::Variable(Variable::Direct(variable)) = access.array.unparenthesized() {
                true
            } else {
                false
            }
        }
        Expression::Variable(Variable::Direct(variable)) => {
            true
        }
        Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
            has_user_input(binary.lhs) || has_user_input(binary.rhs)
        }
        Expression::CompositeString(string) => {
            string
                .parts()
                .iter()
                .any(|part| matches!(part, StringPart::Expression(_) | StringPart::BracedExpression(_)))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_local_file_inclusion() {
        let violations = analyze_file_for_rule("e105/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e105/safe.php", CODE).is_empty());
    }
}
