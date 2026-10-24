use std::collections::HashMap;
use std::path::{Path, PathBuf};

use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;

use super::contracts::{self, Docblock};

#[derive(Clone, Debug, PartialEq)]
pub enum ConstantValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Null,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FunctionSummary {
    pub return_value: Option<ConstantValue>,
    pub return_parameter: Option<usize>,
    pub asserts: Vec<ParameterAssertion>,
    pub never_returns: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterAssertion {
    pub index: usize,
    pub ty: String,
    pub negated: bool,
}

#[derive(Default)]
pub struct SummaryIndex {
    files: HashMap<PathBuf, FileSummaries>,
}

#[derive(Clone, Copy, PartialEq)]
enum ParentKind {
    Extends,
    Trait,
}

#[derive(Default)]
struct FileSummaries {
    summaries: Vec<(String, FunctionSummary)>,
    parents: Vec<(String, String, ParentKind)>,
}

impl SummaryIndex {
    pub fn clear(&mut self) {
        self.files.clear();
    }

    pub fn remove_file(&mut self, path: &Path) {
        self.files.remove(path);
    }

    pub fn replace_file(&mut self, file: &File<'_>) {
        self.files.remove(&file.path);

        let Some(program) = file.ast else {
            return;
        };

        let mut summaries = Vec::new();
        let mut parents = Vec::new();
        collect_summaries(
            program.statements.as_slice(),
            program.trivia.as_slice(),
            program.source_text,
            None,
            &mut summaries,
            &mut parents,
        );
        self.files
            .insert(file.path.clone(), FileSummaries { summaries, parents });
    }

    pub fn snapshot(&self) -> HashMap<String, FunctionSummary> {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (name, _) in self.files.values().flat_map(|file| file.summaries.iter()) {
            *counts.entry(name.as_str()).or_default() += 1;
        }

        let mut summaries: HashMap<String, FunctionSummary> = self
            .files
            .values()
            .flat_map(|file| file.summaries.iter())
            .filter(|(name, _)| counts.get(name.as_str()) == Some(&1))
            .map(|(name, summary)| (name.clone(), summary.clone()))
            .collect();

        let parents = self.inheritors();

        for _ in 0..32 {
            let mut additions = Vec::new();
            for (child, inheritors) in &parents {
                let mut candidates: HashMap<String, Option<FunctionSummary>> = HashMap::new();
                for parent in inheritors {
                    let prefix = format!("{parent}::");
                    for (name, summary) in &summaries {
                        let Some(method) = name.strip_prefix(&prefix) else {
                            continue;
                        };
                        let key = format!("{child}::{method}");
                        if summaries.contains_key(&key) {
                            continue;
                        }
                        candidates
                            .entry(key)
                            .and_modify(|candidate| {
                                if candidate.as_ref() != Some(summary) {
                                    *candidate = None;
                                }
                            })
                            .or_insert_with(|| Some(summary.clone()));
                    }
                }
                for (name, summary) in candidates {
                    if let Some(summary) = summary {
                        additions.push((name, summary));
                    }
                }
            }
            if additions.is_empty() {
                break;
            }
            for (name, summary) in additions {
                summaries.entry(name).or_insert(summary);
            }
        }

        summaries
    }

    /// Return the unique parent of each class or trait, when exactly one is known.
    pub fn parent_map(&self) -> HashMap<String, String> {
        let mut parents: HashMap<String, Vec<String>> = HashMap::new();
        for file in self.files.values() {
            for (child, parent, kind) in &file.parents {
                if *kind == ParentKind::Extends {
                    parents
                        .entry(child.clone())
                        .or_default()
                        .push(parent.clone());
                }
            }
        }
        for inheritors in parents.values_mut() {
            inheritors.sort();
            inheritors.dedup();
        }
        parents
            .into_iter()
            .filter_map(|(child, mut parents)| {
                if parents.len() == 1 {
                    Some((child, parents.remove(0)))
                } else {
                    None
                }
            })
            .collect()
    }

    fn inheritors(&self) -> HashMap<String, Vec<String>> {
        let mut parents: HashMap<String, Vec<String>> = HashMap::new();
        for file in self.files.values() {
            for (child, parent, _) in &file.parents {
                parents
                    .entry(child.clone())
                    .or_default()
                    .push(parent.clone());
            }
        }
        for inheritors in parents.values_mut() {
            inheritors.sort();
            inheritors.dedup();
        }
        parents
    }
}

fn collect_summaries(
    statements: &[Statement<'_>],
    trivia: &[Trivia<'_>],
    source: &[u8],
    namespace: Option<&str>,
    summaries: &mut Vec<(String, FunctionSummary)>,
    parents: &mut Vec<(String, String, ParentKind)>,
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
                    trivia,
                    source,
                    name.as_deref(),
                    summaries,
                    parents,
                );
            }
            Statement::Function(function) => {
                let function_name =
                    String::from_utf8_lossy(function.name.value).to_ascii_lowercase();
                summaries.push((
                    qualify(namespace, &function_name),
                    summarize_function(
                        function.body.statements.as_slice(),
                        &function.parameter_list,
                        function.return_type_hint.is_none(),
                        function
                            .return_type_hint
                            .as_ref()
                            .is_some_and(|hint| hint_is_never(&hint.hint)),
                        &docblock_before(trivia, source, function.span()),
                    ),
                ));
            }
            Statement::Class(class) => {
                let class_name = qualify(
                    namespace,
                    &String::from_utf8_lossy(class.name.value).to_ascii_lowercase(),
                );
                if let Some(extends) = &class.extends {
                    for parent in extends.types.iter() {
                        parents.push((
                            class_name.clone(),
                            resolve_name(parent, namespace),
                            ParentKind::Extends,
                        ));
                    }
                }
                collect_member_summaries(
                    namespace,
                    &String::from_utf8_lossy(class.name.value),
                    &class.members,
                    trivia,
                    source,
                    summaries,
                    parents,
                );
            }
            Statement::Interface(interface) => {
                let class_name = qualify(
                    namespace,
                    &String::from_utf8_lossy(interface.name.value).to_ascii_lowercase(),
                );
                if let Some(extends) = &interface.extends {
                    for parent in extends.types.iter() {
                        parents.push((
                            class_name.clone(),
                            resolve_name(parent, namespace),
                            ParentKind::Extends,
                        ));
                    }
                }
                collect_member_summaries(
                    namespace,
                    &String::from_utf8_lossy(interface.name.value),
                    &interface.members,
                    trivia,
                    source,
                    summaries,
                    parents,
                );
            }
            Statement::Trait(trait_) => {
                collect_member_summaries(
                    namespace,
                    &String::from_utf8_lossy(trait_.name.value),
                    &trait_.members,
                    trivia,
                    source,
                    summaries,
                    parents,
                );
            }
            Statement::Enum(enum_) => {
                collect_member_summaries(
                    namespace,
                    &String::from_utf8_lossy(enum_.name.value),
                    &enum_.members,
                    trivia,
                    source,
                    summaries,
                    parents,
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
    trivia: &[Trivia<'_>],
    source: &[u8],
    summaries: &mut Vec<(String, FunctionSummary)>,
    parents: &mut Vec<(String, String, ParentKind)>,
) {
    let class = qualify(namespace, &class.to_ascii_lowercase());
    for member in members.iter() {
        if let ClassLikeMember::TraitUse(trait_use) = member {
            for trait_name in trait_use.trait_names.iter() {
                parents.push((
                    class.clone(),
                    resolve_name(trait_name, namespace),
                    ParentKind::Trait,
                ));
            }
        } else if let ClassLikeMember::Method(method) = member {
            if let MethodBody::Concrete(block) = &method.body {
                let method_name = String::from_utf8_lossy(method.name.value).to_ascii_lowercase();
                summaries.push((
                    format!("{class}::{method_name}"),
                    summarize_function(
                        block.statements.as_slice(),
                        &method.parameter_list,
                        method.return_type_hint.is_none(),
                        method
                            .return_type_hint
                            .as_ref()
                            .is_some_and(|hint| hint_is_never(&hint.hint)),
                        &docblock_before(trivia, source, method.span()),
                    ),
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

fn resolve_name(identifier: &Identifier<'_>, namespace: Option<&str>) -> String {
    let name = String::from_utf8_lossy(identifier.value()).to_ascii_lowercase();
    match identifier {
        Identifier::FullyQualified(_) => name.trim_start_matches('\\').to_string(),
        Identifier::Local(_) | Identifier::Qualified(_) => qualify(namespace, &name),
    }
}

fn summarize_function(
    statements: &[Statement<'_>],
    parameters: &FunctionLikeParameterList<'_>,
    untyped_return: bool,
    never_returns: bool,
    docblock: &Docblock,
) -> FunctionSummary {
    let asserts = parameter_assertions(parameters, docblock);

    if untyped_return {
        if let Some(index) = identity_parameter(statements, parameters) {
            return FunctionSummary {
                return_value: None,
                return_parameter: Some(index),
                asserts,
                never_returns,
            };
        }
    }

    let mut returns = Vec::new();
    collect_returns(statements, &mut returns);

    let mut value = None;
    for expression in returns {
        let constant = expression.and_then(constant_value);
        match (&value, constant) {
            (None, Some(constant)) if !expression.is_none() => value = Some(constant),
            (Some(previous), Some(constant)) if previous == &constant => {}
            _ => {
                return FunctionSummary {
                    asserts,
                    never_returns,
                    ..FunctionSummary::default()
                };
            }
        }
    }

    FunctionSummary {
        return_value: value,
        return_parameter: None,
        asserts,
        never_returns,
    }
}

fn hint_is_never(hint: &Hint<'_>) -> bool {
    matches!(hint, Hint::Never(_))
}

fn parameter_assertions(
    parameters: &FunctionLikeParameterList<'_>,
    docblock: &Docblock,
) -> Vec<ParameterAssertion> {
    docblock
        .asserts
        .iter()
        .filter_map(|assertion| {
            let index = parameters.parameters.iter().position(|parameter| {
                String::from_utf8_lossy(parameter.variable.name) == assertion.parameter
            })?;
            Some(ParameterAssertion {
                index,
                ty: assertion.ty.clone(),
                negated: assertion.negated,
            })
        })
        .collect()
}

fn docblock_before(trivia: &[Trivia<'_>], source: &[u8], span: mago_span::Span) -> Docblock {
    let mut best = None;
    for trivia in trivia {
        if !trivia.kind.is_docblock() || trivia.span.end.offset > span.start.offset {
            continue;
        }
        if best.is_none_or(|best: &Trivia<'_>| trivia.span.end.offset > best.span.end.offset) {
            best = Some(trivia);
        }
    }
    let Some(trivia) = best else {
        return Docblock::default();
    };
    let start = trivia.span.end.offset as usize;
    let end = span.start.offset as usize;
    if !source
        .get(start..end)
        .is_some_and(|gap| gap.iter().all(u8::is_ascii_whitespace))
    {
        return Docblock::default();
    }
    contracts::parse_docblock(trivia.value)
}

fn identity_parameter(
    statements: &[Statement<'_>],
    parameters: &FunctionLikeParameterList<'_>,
) -> Option<usize> {
    let mut significant = statements
        .iter()
        .filter(|statement| !matches!(statement, Statement::Noop(_)));
    let Statement::Return(return_statement) = significant.next()? else {
        return None;
    };
    if significant.next().is_some() {
        return None;
    }

    let value = return_statement.value?;
    let Expression::Variable(Variable::Direct(variable)) = value.unparenthesized() else {
        return None;
    };

    if parameters.parameters.iter().any(|parameter| {
        parameter.hint.is_some() || parameter.ampersand.is_some() || parameter.ellipsis.is_some()
    }) {
        return None;
    }

    parameters
        .parameters
        .iter()
        .position(|parameter| parameter.variable.name == variable.name)
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

    fn summarize(content: &str) -> HashMap<String, FunctionSummary> {
        let mut index = SummaryIndex::default();
        let arena = LocalArena::new();
        let file = file(&arena, "summary.php", content);
        index.replace_file(&file);
        index.snapshot()
    }

    #[test]
    fn summarizes_unmodified_identity_parameters() {
        let summaries = summarize("<?php function pass($value) { return $value; }");

        assert_eq!(summaries["pass"].return_parameter, Some(0));
        assert_eq!(summaries["pass"].return_value, None);
    }

    #[test]
    fn does_not_summarize_modified_or_reference_parameters() {
        let modified = summarize("<?php function pass($value) { $value = 2; return $value; }");
        assert_eq!(modified["pass"].return_parameter, None);
        assert_eq!(modified["pass"].return_value, None);

        let referenced = summarize("<?php function pass(&$value) { return $value; }");
        assert_eq!(referenced["pass"].return_parameter, None);
        assert_eq!(referenced["pass"].return_value, None);

        let typed = summarize("<?php function pass(int $value): int { return $value; }");
        assert_eq!(typed["pass"].return_parameter, None);

        let typed_return = summarize("<?php function pass($value): int { return $value; }");
        assert_eq!(typed_return["pass"].return_parameter, None);
    }

    #[test]
    fn removing_a_file_drops_only_its_summaries() {
        let mut index = SummaryIndex::default();
        let removed = PathBuf::from("removed.php");

        {
            let arena = LocalArena::new();
            let file = file(
                &arena,
                removed.to_str().unwrap(),
                "<?php function removed_constant(): int { return 0; }",
            );
            index.replace_file(&file);
        }
        {
            let arena = LocalArena::new();
            let file = file(
                &arena,
                "kept.php",
                "<?php function kept_constant(): int { return 2; }",
            );
            index.replace_file(&file);
        }

        index.remove_file(&removed);

        let snapshot = index.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert!(snapshot.contains_key("kept_constant"));
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
