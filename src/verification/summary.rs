use std::collections::HashMap;
use std::path::PathBuf;

use mago_syntax::cst::*;

use crate::file::File;

#[derive(Clone, Debug, PartialEq)]
pub enum ConstantValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Null,
}

#[derive(Clone, Debug, Default)]
pub struct FunctionSummary {
    pub return_value: Option<ConstantValue>,
}

#[derive(Default)]
pub struct SummaryIndex {
    files: HashMap<PathBuf, Vec<(String, FunctionSummary)>>,
}

impl SummaryIndex {
    pub fn clear(&mut self) {
        self.files.clear();
    }

    pub fn replace_file(&mut self, file: &File<'_>) {
        self.files.remove(&file.path);

        let Some(program) = file.ast else {
            return;
        };

        let mut summaries = Vec::new();
        collect_summaries(program.statements.as_slice(), None, &mut summaries);
        self.files.insert(file.path.clone(), summaries);
    }

    pub fn snapshot(&self) -> HashMap<String, FunctionSummary> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (name, _) in self.files.values().flat_map(|summaries| summaries.iter()) {
            *counts.entry(name.as_str()).or_default() += 1;
        }

        self.files
            .values()
            .flat_map(|summaries| summaries.iter())
            .filter(|(name, _)| counts.get(name.as_str()) == Some(&1))
            .map(|(name, summary)| (name.clone(), summary.clone()))
            .collect()
    }
}

fn collect_summaries(
    statements: &[Statement<'_>],
    namespace: Option<&str>,
    summaries: &mut Vec<(String, FunctionSummary)>,
) {
    for statement in statements {
        match statement {
            Statement::Namespace(namespace_statement) => {
                let name = namespace_statement.name.as_ref().map(|name| {
                    String::from_utf8_lossy(name.value())
                        .trim_start_matches('\\')
                        .to_ascii_lowercase()
                });
                collect_summaries(
                    namespace_statement.statements().as_slice(),
                    name.as_deref(),
                    summaries,
                );
            }
            Statement::Function(function) => {
                let function_name =
                    String::from_utf8_lossy(function.name.value).to_ascii_lowercase();
                summaries.push((
                    qualify(namespace, &function_name),
                    summarize_function(function.body.statements.as_slice()),
                ));
            }
            Statement::Class(class) => {
                collect_member_summaries(
                    namespace,
                    &String::from_utf8_lossy(class.name.value),
                    &class.members,
                    summaries,
                );
            }
            Statement::Interface(interface) => {
                collect_member_summaries(
                    namespace,
                    &String::from_utf8_lossy(interface.name.value),
                    &interface.members,
                    summaries,
                );
            }
            Statement::Trait(trait_) => {
                collect_member_summaries(
                    namespace,
                    &String::from_utf8_lossy(trait_.name.value),
                    &trait_.members,
                    summaries,
                );
            }
            Statement::Enum(enum_) => {
                collect_member_summaries(
                    namespace,
                    &String::from_utf8_lossy(enum_.name.value),
                    &enum_.members,
                    summaries,
                );
            }
            _ => {}
        }
    }
}

fn collect_member_summaries(
    namespace: Option<&str>,
    class: &str,
    members: &Sequence<'_, ClassLikeMember<'_>>,
    summaries: &mut Vec<(String, FunctionSummary)>,
) {
    let class = qualify(namespace, &class.to_ascii_lowercase());
    for member in members.iter() {
        if let ClassLikeMember::Method(method) = member {
            if let MethodBody::Concrete(block) = &method.body {
                let method_name = String::from_utf8_lossy(method.name.value).to_ascii_lowercase();
                summaries.push((
                    format!("{class}::{method_name}"),
                    summarize_function(block.statements.as_slice()),
                ));
            }
        }
    }
}

fn qualify(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(namespace) if !namespace.is_empty() => format!("{namespace}\\{name}"),
        _ => name.to_string(),
    }
}

fn summarize_function(statements: &[Statement<'_>]) -> FunctionSummary {
    let mut returns = Vec::new();
    collect_returns(statements, &mut returns);

    let mut value = None;
    for expression in returns {
        let constant = expression.and_then(constant_value);
        match (&value, constant) {
            (None, Some(constant)) if !expression.is_none() => value = Some(constant),
            (Some(previous), Some(constant)) if previous == &constant => {}
            _ => return FunctionSummary::default(),
        }
    }

    FunctionSummary {
        return_value: value,
    }
}

fn collect_returns<'arena>(
    statements: &'arena [Statement<'arena>],
    returns: &mut Vec<Option<&'arena Expression<'arena>>>,
) {
    for statement in statements {
        match statement {
            Statement::Return(return_statement) => returns.push(return_statement.value),
            Statement::Block(block) => collect_returns(block.statements.as_slice(), returns),
            Statement::Namespace(namespace) => {
                collect_returns(namespace.statements().as_slice(), returns)
            }
            Statement::If(if_statement) => {
                collect_if_returns(if_statement, returns);
            }
            Statement::While(while_statement) => match &while_statement.body {
                WhileBody::Statement(statement) => {
                    collect_returns(std::slice::from_ref(statement), returns)
                }
                WhileBody::ColonDelimited(body) => {
                    collect_returns(body.statements.as_slice(), returns)
                }
            },
            Statement::DoWhile(do_while) => {
                collect_returns(std::slice::from_ref(do_while.statement), returns)
            }
            Statement::For(for_statement) => match &for_statement.body {
                ForBody::Statement(statement) => {
                    collect_returns(std::slice::from_ref(statement), returns)
                }
                ForBody::ColonDelimited(body) => {
                    collect_returns(body.statements.as_slice(), returns)
                }
            },
            Statement::Foreach(foreach) => collect_returns(foreach.body.statements(), returns),
            Statement::Try(try_statement) => {
                collect_returns(try_statement.block.statements.as_slice(), returns);
                for catch in try_statement.catch_clauses.iter() {
                    collect_returns(catch.block.statements.as_slice(), returns);
                }
                if let Some(finally) = &try_statement.finally_clause {
                    collect_returns(finally.block.statements.as_slice(), returns);
                }
            }
            Statement::Declare(declare) => match &declare.body {
                DeclareBody::Statement(statement) => {
                    collect_returns(std::slice::from_ref(statement), returns)
                }
                DeclareBody::ColonDelimited(body) => {
                    collect_returns(body.statements.as_slice(), returns)
                }
            },
            _ => {}
        }
    }
}

fn collect_if_returns<'arena>(
    if_statement: &'arena If<'arena>,
    returns: &mut Vec<Option<&'arena Expression<'arena>>>,
) {
    collect_returns(if_statement.body.statements(), returns);
    for (_, statements) in if_statement.body.else_if_clauses() {
        collect_returns(statements, returns);
    }
    if let Some(statements) = if_statement.body.else_statements() {
        collect_returns(statements, returns);
    }
}

fn constant_value(expression: &Expression<'_>) -> Option<ConstantValue> {
    match expression.unparenthesized() {
        Expression::Literal(literal) => match literal {
            Literal::Integer(integer) => integer
                .value
                .and_then(|value| i64::try_from(value).ok())
                .map(ConstantValue::Int),
            Literal::Float(float) => Some(ConstantValue::Float(float.value.into_inner())),
            Literal::String(string) => string
                .value
                .and_then(|value| std::str::from_utf8(value).ok())
                .map(|value| ConstantValue::String(value.to_string())),
            Literal::True(_) => Some(ConstantValue::Bool(true)),
            Literal::False(_) => Some(ConstantValue::Bool(false)),
            Literal::Null(_) => Some(ConstantValue::Null),
        },
        Expression::UnaryPrefix(unary) => match unary.operator {
            UnaryPrefixOperator::Negation(_) => match constant_value(unary.operand)? {
                ConstantValue::Int(value) => Some(ConstantValue::Int(value.checked_neg()?)),
                ConstantValue::Float(value) => Some(ConstantValue::Float(-value)),
                _ => None,
            },
            UnaryPrefixOperator::Plus(_) => constant_value(unary.operand),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use mago_allocator::prelude::LocalArena;

    use super::*;

    fn file<'arena>(arena: &'arena LocalArena, path: &str, content: &str) -> File<'arena> {
        File::new(arena, PathBuf::from(path), content.to_string())
    }

    #[test]
    fn ambiguous_definitions_are_omitted_and_reindexing_keeps_other_files() {
        let mut index = SummaryIndex::default();

        {
            let arena = LocalArena::new();
            let file = file(
                &arena,
                "ambiguous_a.php",
                "<?php function duplicate_constant(): int { return 2; }",
            );
            index.replace_file(&file);
        }
        {
            let arena = LocalArena::new();
            let file = file(
                &arena,
                "ambiguous_b.php",
                "<?php function duplicate_constant(): int { return 2; }",
            );
            index.replace_file(&file);
        }

        assert!(index.snapshot().is_empty());

        {
            let arena = LocalArena::new();
            let file = file(&arena, "ambiguous_a.php", "<?php");
            index.replace_file(&file);
        }

        let snapshot = index.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(
            snapshot["duplicate_constant"].return_value,
            Some(ConstantValue::Int(2))
        );
    }
}
