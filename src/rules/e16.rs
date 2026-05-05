use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;

pub(crate) static CODE: &str = "E0016";
static DESCRIPTION: &str = "Cognitive complexity";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_complexity: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_complexity: 15 }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        true
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        };
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    if let MethodBody::Concrete(block) = &method.body {
                        let complexity = calculate_cognitive_complexity(&block.statements, 0);

                        if complexity > self.settings.max_complexity {
                            let suggestion = format!(
                                "The body of {} method has {} cognitive complexity. Make it easier to understand.",
                                method.name.value,
                                complexity,
                            );
                            violations.push(self.new_violation(file, suggestion, method.span()));
                        }
                    }
                }
            }
        }
        violations
    }
}

fn calculate_cognitive_complexity(statements: &Sequence<'_, Statement<'_>>, nesting: i64) -> i64 {
    let mut complexity = 0;
    for statement in statements.iter() {
        complexity += calculate_statement_cognitive_complexity(statement, nesting);
    }
    complexity
}

fn calculate_statement_cognitive_complexity(statement: &Statement<'_>, nesting: i64) -> i64 {
    let mut complexity = 0;
    match statement {
        Statement::If(if_stmt) => {
            // Incremented for 'if' (+1) plus nesting level
            complexity += 1 + nesting;
            match &if_stmt.body {
                IfBody::Statement(body) => {
                    complexity +=
                        calculate_statement_cognitive_complexity(&body.statement, nesting + 1);
                    for clause in body.else_if_clauses.iter() {
                        // 'else if' increments by 1 but NOT nesting (it's part of the same level)
                        complexity += 1;
                        complexity += calculate_statement_cognitive_complexity(
                            &clause.statement,
                            nesting + 1,
                        );
                    }
                    if let Some(else_clause) = &body.else_clause {
                        // 'else' increments by 1
                        complexity += 1;
                        complexity += calculate_statement_cognitive_complexity(
                            &else_clause.statement,
                            nesting + 1,
                        );
                    }
                }
                IfBody::ColonDelimited(body) => {
                    complexity += calculate_cognitive_complexity(&body.statements, nesting + 1);
                    for clause in body.else_if_clauses.iter() {
                        complexity += 1;
                        for s in clause.statements.iter() {
                            complexity += calculate_statement_cognitive_complexity(s, nesting + 1);
                        }
                    }
                    if let Some(else_clause) = &body.else_clause {
                        complexity += 1;
                        for s in else_clause.statements.iter() {
                            complexity += calculate_statement_cognitive_complexity(s, nesting + 1);
                        }
                    }
                }
            }
            // Also check condition for boolean sequences
            complexity += calculate_expression_complexity(if_stmt.condition);
        }
        Statement::While(while_stmt) => {
            complexity += 1 + nesting;
            match &while_stmt.body {
                WhileBody::Statement(body) => {
                    complexity += calculate_statement_cognitive_complexity(body, nesting + 1);
                }
                WhileBody::ColonDelimited(body) => {
                    complexity += calculate_cognitive_complexity(&body.statements, nesting + 1);
                }
            }
            complexity += calculate_expression_complexity(while_stmt.condition);
        }
        Statement::DoWhile(do_while_stmt) => {
            complexity += 1 + nesting;
            complexity +=
                calculate_statement_cognitive_complexity(&do_while_stmt.statement, nesting + 1);
            complexity += calculate_expression_complexity(do_while_stmt.condition);
        }
        Statement::For(for_stmt) => {
            complexity += 1 + nesting;
            match &for_stmt.body {
                ForBody::Statement(body) => {
                    complexity += calculate_statement_cognitive_complexity(body, nesting + 1);
                }
                ForBody::ColonDelimited(body) => {
                    complexity += calculate_cognitive_complexity(&body.statements, nesting + 1);
                }
            }
            for expr in for_stmt.conditions.iter() {
                complexity += calculate_expression_complexity(expr);
            }
        }
        Statement::Foreach(foreach_stmt) => {
            complexity += 1 + nesting;
            match &foreach_stmt.body {
                ForeachBody::Statement(body) => {
                    complexity += calculate_statement_cognitive_complexity(body, nesting + 1);
                }
                ForeachBody::ColonDelimited(body) => {
                    complexity += calculate_cognitive_complexity(&body.statements, nesting + 1);
                }
            }
        }
        Statement::Switch(switch_stmt) => {
            complexity += 1 + nesting;
            let cases = match &switch_stmt.body {
                SwitchBody::BraceDelimited(body) => &body.cases,
                SwitchBody::ColonDelimited(body) => &body.cases,
            };
            for case in cases.iter() {
                match case {
                    SwitchCase::Expression(c) => {
                        complexity += calculate_cognitive_complexity(&c.statements, nesting + 1);
                    }
                    SwitchCase::Default(c) => {
                        complexity += calculate_cognitive_complexity(&c.statements, nesting + 1);
                    }
                }
            }
        }
        Statement::Try(try_stmt) => {
            // 'try' itself doesn't increment usually, but 'catch' does
            complexity += calculate_cognitive_complexity(&try_stmt.block.statements, nesting);
            for catch in try_stmt.catch_clauses.iter() {
                complexity += 1 + nesting;
                complexity += calculate_cognitive_complexity(&catch.block.statements, nesting + 1);
            }
            if let Some(finally) = &try_stmt.finally_clause {
                complexity += calculate_cognitive_complexity(&finally.block.statements, nesting);
            }
        }
        Statement::Block(block) => {
            complexity += calculate_cognitive_complexity(&block.statements, nesting);
        }
        Statement::Expression(expr_stmt) => {
            complexity += calculate_expression_complexity(expr_stmt.expression);
        }
        Statement::Return(ret_stmt) => {
            if let Some(expr) = ret_stmt.value {
                complexity += calculate_expression_complexity(expr);
            }
        }
        _ => {}
    }
    complexity
}

fn calculate_expression_complexity(expression: &Expression<'_>) -> i64 {
    let mut complexity = 0;
    match expression {
        Expression::Conditional(cond) => {
            complexity += 1;
            complexity += calculate_expression_complexity(cond.condition);
            if let Some(then_expr) = cond.then {
                complexity += calculate_expression_complexity(then_expr);
            }
            complexity += calculate_expression_complexity(cond.r#else);
        }
        Expression::Binary(bin) => {
            if is_boolean_operator(bin.operator) {
                complexity += 1;
                // Sequences of the SAME operator don't increment further?
                // Actually SonarSource says:
                // a && b && c -> 1
                // a && b || c -> 2
                // We'd need to track the previous operator. For simplicity now, let's just do basic increment.
            }
            complexity += calculate_expression_complexity(bin.lhs);
            complexity += calculate_expression_complexity(bin.rhs);
        }
        Expression::Parenthesized(p) => {
            complexity += calculate_expression_complexity(p.expression);
        }
        Expression::Call(call) => match call {
            Call::Method(m) => {
                for arg in m.argument_list.arguments.iter() {
                    complexity += calculate_expression_complexity(arg.value());
                }
            }
            Call::NullSafeMethod(m) => {
                for arg in m.argument_list.arguments.iter() {
                    complexity += calculate_expression_complexity(arg.value());
                }
            }
            Call::StaticMethod(m) => {
                for arg in m.argument_list.arguments.iter() {
                    complexity += calculate_expression_complexity(arg.value());
                }
            }
            Call::Function(f) => {
                for arg in f.argument_list.arguments.iter() {
                    complexity += calculate_expression_complexity(arg.value());
                }
            }
        },
        _ => {}
    }
    complexity
}

fn is_boolean_operator(operator: BinaryOperator) -> bool {
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
    fn complex() {
        let violations = analyze_file_for_rule("e16/complex.php", CODE);

        assert!(violations.len().gt(&0));
        assert!(violations
            .first()
            .unwrap()
            .suggestion
            .contains("cognitive complexity"));
    }

    #[test]
    fn simple() {
        let violations = analyze_file_for_rule("e16/simple.php", CODE);
        assert_eq!(violations.len(), 0);
    }
}
