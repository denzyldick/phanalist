use std::collections::HashSet;

use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as _;
use crate::rules::ast_support::function_name;

pub(crate) static CODE: &str = "E0050";
static DESCRIPTION: &str = "Position check used as boolean";

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
        let mut seen = HashSet::new();
        self.check_statements(
            file,
            program.statements.as_slice(),
            &mut violations,
            &mut seen,
        );
        Some(violations)
    }
}

impl Rule {
    fn check_statements(
        &self,
        file: &File<'_>,
        statements: &[Statement<'_>],
        violations: &mut Vec<Violation>,
        seen: &mut HashSet<u32>,
    ) {
        for statement in statements {
            self.check_statement(file, statement, violations, seen);
        }
    }

    fn check_statement(
        &self,
        file: &File<'_>,
        statement: &Statement<'_>,
        violations: &mut Vec<Violation>,
        seen: &mut HashSet<u32>,
    ) {
        match statement {
            Statement::If(statement) => {
                self.check_condition(file, statement.condition, violations, seen);
                for (condition, statements) in statement.body.else_if_clauses() {
                    self.check_condition(file, condition, violations, seen);
                    self.check_statements(file, statements, violations, seen);
                }
                self.check_statements(file, statement.body.statements(), violations, seen);
                if let Some(statements) = statement.body.else_statements() {
                    self.check_statements(file, statements, violations, seen);
                }
            }
            Statement::While(statement) => {
                self.check_condition(file, statement.condition, violations, seen);
                self.check_statements(file, statement.body.statements(), violations, seen);
            }
            Statement::DoWhile(statement) => {
                self.check_statement(file, statement.statement, violations, seen);
                self.check_condition(file, statement.condition, violations, seen);
            }
            Statement::For(statement) => {
                for condition in statement.conditions.iter() {
                    self.check_condition(file, condition, violations, seen);
                }
                self.check_statements(file, statement.body.statements(), violations, seen);
            }
            Statement::Foreach(statement) => {
                self.check_statements(file, statement.body.statements(), violations, seen);
            }
            Statement::Switch(statement) => {
                for case in statement.body.cases().iter() {
                    self.check_statements(file, case.statements(), violations, seen);
                }
            }
            Statement::Block(statement) => {
                self.check_statements(file, statement.statements.as_slice(), violations, seen);
            }
            Statement::Namespace(statement) => {
                self.check_statements(file, statement.statements().as_slice(), violations, seen);
            }
            Statement::Try(statement) => {
                self.check_statements(
                    file,
                    statement.block.statements.as_slice(),
                    violations,
                    seen,
                );
                for catch in statement.catch_clauses.iter() {
                    self.check_statements(
                        file,
                        catch.block.statements.as_slice(),
                        violations,
                        seen,
                    );
                }
                if let Some(finally) = &statement.finally_clause {
                    self.check_statements(
                        file,
                        finally.block.statements.as_slice(),
                        violations,
                        seen,
                    );
                }
            }
            Statement::Function(statement) => {
                self.check_statements(file, statement.body.statements.as_slice(), violations, seen);
            }
            Statement::Class(statement) => {
                self.check_members(file, statement.members.as_slice(), violations, seen);
            }
            Statement::Interface(statement) => {
                self.check_members(file, statement.members.as_slice(), violations, seen);
            }
            Statement::Trait(statement) => {
                self.check_members(file, statement.members.as_slice(), violations, seen);
            }
            Statement::Enum(statement) => {
                self.check_members(file, statement.members.as_slice(), violations, seen);
            }
            _ => {}
        }
    }

    fn check_members(
        &self,
        file: &File<'_>,
        members: &[ClassLikeMember<'_>],
        violations: &mut Vec<Violation>,
        seen: &mut HashSet<u32>,
    ) {
        for member in members {
            if let ClassLikeMember::Method(method) = member {
                if let MethodBody::Concrete(block) = &method.body {
                    self.check_statements(file, block.statements.as_slice(), violations, seen);
                }
            }
        }
    }

    fn check_condition(
        &self,
        file: &File<'_>,
        expression: &Expression<'_>,
        violations: &mut Vec<Violation>,
        seen: &mut HashSet<u32>,
    ) {
        let expression = expression.unparenthesized();

        match expression {
            Expression::Binary(binary) if is_comparison(&binary.operator) => {}
            Expression::Binary(binary) if is_boolean(&binary.operator) => {
                self.check_condition(file, binary.lhs, violations, seen);
                self.check_condition(file, binary.rhs, violations, seen);
            }
            Expression::UnaryPrefix(unary)
                if matches!(unary.operator, UnaryPrefixOperator::Not(_)) =>
            {
                self.check_condition(file, unary.operand, violations, seen);
            }
            Expression::Conditional(conditional) => {
                self.check_condition(file, conditional.condition, violations, seen);
            }
            Expression::Call(Call::Function(call)) => {
                let Some(name) = function_name(call.function) else {
                    return;
                };
                if !is_position_function(&name) {
                    return;
                }

                if !seen.insert(call.span().start.offset) {
                    return;
                }

                let message = Message::new(
                    format!("E0050:position-check-used-as-boolean:{name}"),
                    "{name}() returns 0 for a match at the start of the string. Compare its result with === false or !== false instead of using it as a boolean.",
                )
                .arg("name", name);

                violations.push(self.new_violation(file, message, call.span()));
            }
            _ => {}
        }
    }
}

fn is_position_function(name: &str) -> bool {
    matches!(name, "strpos" | "stripos" | "strrpos" | "strripos")
}

fn is_comparison(operator: &BinaryOperator<'_>) -> bool {
    matches!(
        operator,
        BinaryOperator::Equal(_)
            | BinaryOperator::NotEqual(_)
            | BinaryOperator::Identical(_)
            | BinaryOperator::NotIdentical(_)
            | BinaryOperator::AngledNotEqual(_)
            | BinaryOperator::LessThan(_)
            | BinaryOperator::LessThanOrEqual(_)
            | BinaryOperator::GreaterThan(_)
            | BinaryOperator::GreaterThanOrEqual(_)
            | BinaryOperator::Spaceship(_)
            | BinaryOperator::Instanceof(_)
    )
}

fn is_boolean(operator: &BinaryOperator<'_>) -> bool {
    matches!(
        operator,
        BinaryOperator::And(_)
            | BinaryOperator::Or(_)
            | BinaryOperator::LowAnd(_)
            | BinaryOperator::LowOr(_)
            | BinaryOperator::LowXor(_)
    )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_position_checks_used_as_booleans() {
        let violations = analyze_file_for_rule("e50/truthiness_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_explicit_false_comparisons() {
        assert!(analyze_file_for_rule("e50/comparison_safe.php", CODE).is_empty());
    }
}
