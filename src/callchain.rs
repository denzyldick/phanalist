//! Lightweight call-chain evidence for generated tests and reports.
//!
//! This is deliberately separate from the taint call graph. It does not try to
//! prove whole-program reachability; it records direct calls between declared
//! functions/methods and framework route attributes so a finding can say why
//! it matters:
//!
//! ```text
//! POST /checkout → CheckoutController::submit() → DiscountCalculator::discountedTotal()
//! ```
//!
//! Anything it cannot resolve safely is simply omitted from the test docblock.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::rules::ast_support::{literal_string, walk_statement_expressions};

const MAX_CHAIN_DEPTH: usize = 8;
const MAX_CHAINS: usize = 2;

/// A direct call from one declared function/method to another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSite {
    /// Fully qualified caller name, e.g. `App\Controller\CheckoutController::submit`.
    pub caller: String,
    /// Route label when the caller itself is a route action.
    pub route: Option<String>,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Default)]
pub struct CallChainIndex {
    graph: Mutex<CallChainGraph>,
}

#[derive(Clone, Debug)]
struct Declaration {
    name: String,
    route: Option<String>,
    file: PathBuf,
    start_line: usize,
    end_line: usize,
}

#[derive(Default)]
struct CallChainGraph {
    exact: HashMap<String, Vec<CallSite>>,
    tail: HashMap<String, Vec<CallSite>>,
    declarations: Vec<Declaration>,
}

impl CallChainIndex {
    pub fn reset(&self) {
        if let Ok(mut graph) = self.graph.lock() {
            *graph = CallChainGraph::default();
        }
    }

    /// Index declarations and direct calls in one file.
    pub fn index_file(&self, file: &File<'_>) {
        let Some(program) = file.ast else {
            return;
        };
        let graph = Self::collect(program, file);
        if let Ok(mut current) = self.graph.lock() {
            for (callee, sites) in graph.exact {
                current.exact.entry(callee).or_default().extend(sites);
            }
            for (callee, sites) in graph.tail {
                current.tail.entry(callee).or_default().extend(sites);
            }
            current.declarations.extend(graph.declarations);
        }
    }

    /// Infer route-rooted call chains that reach `target`.
    pub fn chain_for(&self, target: &str, target_label: &str) -> Vec<String> {
        let Ok(graph) = self.graph.lock() else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back((target.to_string(), Vec::<CallSite>::new(), 0usize));
        let mut visited = HashSet::new();
        visited.insert(normalize(target));

        while let Some((current, chain, depth)) = queue.pop_front() {
            let callers = graph.callers(&current);
            for site in callers {
                let key = normalize(&site.caller);
                if !visited.insert(key.clone()) {
                    continue;
                }

                let mut next_chain = chain.clone();
                next_chain.push(site.clone());

                if let Some(route) = &site.route {
                    candidates.push(format_chain(route, &next_chain, target_label));
                    if candidates.len() == MAX_CHAINS {
                        return candidates;
                    }
                    continue;
                }

                if depth + 1 < MAX_CHAIN_DEPTH {
                    queue.push_back((site.caller.clone(), next_chain, depth + 1));
                }
            }
        }

        candidates
    }

    /// Infer a chain for a finding line when no witness/callable is available.
    pub fn chain_for_location(&self, path: &Path, line: usize) -> Vec<String> {
        let declaration = {
            let Ok(graph) = self.graph.lock() else {
                return Vec::new();
            };
            let Some(declaration) = graph.declaration_for(path, line) else {
                return Vec::new();
            };
            declaration.clone()
        };
        let target = format!(
            "{}() at {}:{}",
            declaration.name,
            declaration.file.display(),
            line
        );
        if let Some(route) = &declaration.route {
            return vec![format_chain(route, &[], &target)];
        }
        self.chain_for(&declaration.name, &target)
    }

    fn collect<'arena>(program: &'arena Program<'arena>, file: &File<'_>) -> CallChainGraph {
        let mut graph = CallChainGraph::default();
        let mut ctx = Context::default();
        collect_statements(
            program.statements.as_slice(),
            file,
            &mut ctx,
            &mut graph,
            None,
            None,
        );
        graph
    }
}

impl CallChainGraph {
    fn insert(&mut self, callee: String, site: CallSite) {
        self.exact
            .entry(normalize(&callee))
            .or_default()
            .push(site.clone());
        self.tail.entry(tail_key(&callee)).or_default().push(site);
    }

    fn callers(&self, callee: &str) -> Vec<CallSite> {
        let exact = normalize(callee);
        let tail = tail_key(callee);
        let mut sites = self.exact.get(&exact).cloned().unwrap_or_default();
        if tail != exact {
            sites.extend(self.tail.get(&tail).cloned().unwrap_or_default());
        }
        dedupe(sites)
    }

    fn declaration_for(&self, path: &Path, line: usize) -> Option<&Declaration> {
        self.declarations
            .iter()
            .filter(|declaration| {
                same_path(&declaration.file, path)
                    && line >= declaration.start_line
                    && line <= declaration.end_line
            })
            .min_by_key(|declaration| declaration.end_line - declaration.start_line)
    }
}

#[derive(Default)]
struct Context {
    namespace: String,
    imports: HashMap<String, String>,
    property_types: HashMap<String, String>,
}

#[derive(Clone)]
struct BodyContext<'a> {
    name: String,
    class: Option<String>,
    route: Option<String>,
    properties: &'a HashMap<String, String>,
    imports: &'a HashMap<String, String>,
    namespace: &'a str,
}

fn collect_statements<'arena>(
    statements: &'arena [Statement<'arena>],
    file: &File<'_>,
    ctx: &mut Context,
    graph: &mut CallChainGraph,
    class: Option<&str>,
    class_route: Option<&str>,
) {
    for statement in statements {
        match statement {
            Statement::Namespace(namespace) => {
                let previous_namespace = std::mem::replace(
                    &mut ctx.namespace,
                    identifier_string(namespace.name.map(|name| name.value())).unwrap_or_default(),
                );
                collect_statements(
                    namespace.statements().as_slice(),
                    file,
                    ctx,
                    graph,
                    class,
                    class_route,
                );
                ctx.namespace = previous_namespace;
            }
            Statement::Use(use_statement) => collect_use(use_statement, ctx),
            Statement::Class(class_declaration) => {
                collect_class(class_declaration, file, ctx, graph);
            }
            Statement::Interface(interface) => {
                collect_class_members(
                    &interface.members,
                    file,
                    ctx,
                    graph,
                    &class_name(ctx, b"", &interface.name.value),
                    None,
                );
            }
            Statement::Trait(trait_declaration) => {
                collect_class_members(
                    &trait_declaration.members,
                    file,
                    ctx,
                    graph,
                    &class_name(ctx, b"", &trait_declaration.name.value),
                    None,
                );
            }
            Statement::Function(function) => {
                let name = qualified(
                    ctx,
                    &identifier_string(Some(function.name.value)).unwrap_or_default(),
                );
                let route = route_label(&function.attribute_lists);
                let span = function.span();
                graph.declarations.push(Declaration {
                    name: name.clone(),
                    route: route.clone(),
                    file: file.path.clone(),
                    start_line: file.line_number(span.start.offset),
                    end_line: file.line_number(span.end.offset),
                });
                collect_body(
                    function.body.statements.as_slice(),
                    file,
                    graph,
                    BodyContext {
                        name,
                        class: None,
                        route,
                        properties: &ctx.property_types,
                        imports: &ctx.imports,
                        namespace: &ctx.namespace,
                    },
                );
            }
            _ => collect_body(
                std::slice::from_ref(statement),
                file,
                graph,
                BodyContext {
                    name: "global".to_string(),
                    class: class.map(str::to_string),
                    route: class_route.map(str::to_string),
                    properties: &ctx.property_types,
                    imports: &ctx.imports,
                    namespace: &ctx.namespace,
                },
            ),
        }
    }
}

fn collect_class<'arena>(
    class: &'arena Class<'arena>,
    file: &File<'_>,
    ctx: &mut Context,
    graph: &mut CallChainGraph,
) {
    let qualified_class = class_name(ctx, b"", &class.name.value);
    let route = route_label(&class.attribute_lists);

    // Property types must be known before method bodies are walked so
    // `$this->service->method()` can be resolved in either declaration order.
    for member in class.members.iter() {
        match member {
            ClassLikeMember::Property(property) => {
                collect_property_type(property, ctx, &qualified_class)
            }
            ClassLikeMember::Method(method) if method.name.value == b"__construct" => {
                for parameter in method.parameter_list.parameters.iter() {
                    if !parameter.is_promoted_property() {
                        continue;
                    }
                    let Some(hint) = &parameter.hint else {
                        continue;
                    };
                    let Some(class) = class_from_hint(hint, ctx) else {
                        continue;
                    };
                    let name = String::from_utf8_lossy(parameter.variable.name)
                        .trim_start_matches('$')
                        .to_string();
                    ctx.property_types
                        .insert(format!("{qualified_class}::{name}"), class);
                }
            }
            _ => {}
        }
    }

    // Property types are keyed by fully qualified class, so they can stay in
    // the file-wide map without leaking into another class's lookups.
    collect_class_members(
        &class.members,
        file,
        ctx,
        graph,
        &qualified_class,
        route.as_deref(),
    );
}

fn collect_class_members<'arena>(
    members: &'arena Sequence<'arena, ClassLikeMember<'arena>>,
    file: &File<'_>,
    ctx: &Context,
    graph: &mut CallChainGraph,
    qualified_class: &str,
    class_route: Option<&str>,
) {
    for member in members.iter() {
        let ClassLikeMember::Method(method) = member else {
            continue;
        };
        let MethodBody::Concrete(body) = &method.body else {
            continue;
        };
        let name = format!(
            "{}::{}",
            qualified_class,
            String::from_utf8_lossy(method.name.value)
        );
        let route =
            route_label(&method.attribute_lists).or_else(|| class_route.map(str::to_string));
        let span = method.span();
        graph.declarations.push(Declaration {
            name: name.clone(),
            route: route.clone(),
            file: file.path.clone(),
            start_line: file.line_number(span.start.offset),
            end_line: file.line_number(span.end.offset),
        });
        collect_body(
            body.statements.as_slice(),
            file,
            graph,
            BodyContext {
                name,
                class: Some(qualified_class.to_string()),
                route,
                properties: &ctx.property_types,
                imports: &ctx.imports,
                namespace: &ctx.namespace,
            },
        );
    }
}

fn collect_property_type(property: &Property<'_>, ctx: &mut Context, qualified_class: &str) {
    let hint = match property {
        Property::Plain(plain) => plain.hint.as_ref(),
        Property::Hooked(hooked) => hooked.hint.as_ref(),
    };
    let Some(hint) = hint else {
        return;
    };
    let Some(class) = class_from_hint(hint, ctx) else {
        return;
    };
    let items: Vec<_> = match property {
        Property::Plain(plain) => plain.items.iter().collect(),
        Property::Hooked(hooked) => vec![&hooked.item],
    };
    for item in items {
        let variable = match item {
            PropertyItem::Abstract(item) => &item.variable,
            PropertyItem::Concrete(item) => &item.variable,
        };
        let name = String::from_utf8_lossy(variable.name)
            .trim_start_matches('$')
            .to_string();
        ctx.property_types
            .insert(format!("{qualified_class}::{name}"), class.clone());
    }
}

fn collect_body<'arena>(
    statements: &'arena [Statement<'arena>],
    file: &File<'_>,
    graph: &mut CallChainGraph,
    ctx: BodyContext<'_>,
) {
    let mut visit = |expression: &'arena Expression<'arena>| {
        let Expression::Call(call) = expression else {
            return;
        };
        let Some(callee) = resolve_callee(call, &ctx) else {
            return;
        };
        graph.insert(
            callee,
            CallSite {
                caller: ctx.name.clone(),
                route: ctx.route.clone(),
                file: file.path.clone(),
                line: file.line_number(call.span().start.offset),
            },
        );
    };
    for statement in statements {
        walk_statement_expressions(statement, &mut visit);
    }
}

fn collect_use(statement: &Use<'_>, ctx: &mut Context) {
    let items: Vec<&UseItem<'_>> = match &statement.items {
        UseItems::Sequence(items) => items.items.iter().collect(),
        UseItems::TypedSequence(items) => items.items.iter().collect(),
        UseItems::TypedList(items) => items.items.iter().collect(),
        UseItems::MixedList(items) => items.items.iter().map(|item| &item.item).collect(),
    };
    for item in items {
        let full = identifier_string(Some(item.name.value())).unwrap_or_default();
        let alias = item
            .alias
            .as_ref()
            .map(|alias| String::from_utf8_lossy(alias.identifier.value).to_string())
            .unwrap_or_else(|| tail_key(&full));
        ctx.imports
            .insert(alias.to_ascii_lowercase(), trim_leading_slash(&full));
    }
}

fn resolve_callee(call: &Call<'_>, ctx: &BodyContext<'_>) -> Option<String> {
    match call {
        Call::Function(call) => {
            let name = expression_identifier(call.function)?;
            Some(resolve_identifier(&name, ctx.imports, ctx.namespace))
        }
        Call::StaticMethod(call) => {
            let class = expression_class(call.class, None, ctx)?;
            let method = selector_text(&call.method)?;
            Some(format!("{class}::{method}"))
        }
        Call::Method(call) => {
            let class = expression_class(call.object, Some(&call.method), ctx)?;
            let method = selector_text(&call.method)?;
            Some(format!("{class}::{method}"))
        }
        Call::NullSafeMethod(call) => {
            let class = expression_class(call.object, Some(&call.method), ctx)?;
            let method = selector_text(&call.method)?;
            Some(format!("{class}::{method}"))
        }
    }
}

fn expression_class<'a>(
    expression: &'a Expression<'a>,
    selector: Option<&ClassLikeMemberSelector<'a>>,
    ctx: &BodyContext<'_>,
) -> Option<String> {
    match expression.unparenthesized() {
        Expression::Identifier(_) => expression_identifier(expression)
            .map(|name| resolve_identifier(&name, ctx.imports, ctx.namespace)),
        Expression::Variable(variable) => {
            let raw = String::from_utf8_lossy(variable_name(variable)?);
            let name = raw.trim_start_matches('$');
            if name == "this" {
                ctx.class.clone()
            } else {
                None
            }
        }
        Expression::Access(Access::Property(access)) => {
            let owner = expression_class(access.object, Some(&access.property), ctx)?;
            let property = selector_text(&access.property)?;
            ctx.properties.get(&format!("{owner}::{property}")).cloned()
        }
        Expression::Access(Access::NullSafeProperty(access)) => {
            let owner = expression_class(access.object, Some(&access.property), ctx)?;
            let property = selector_text(&access.property)?;
            ctx.properties.get(&format!("{owner}::{property}")).cloned()
        }
        Expression::Call(_) => None,
        _ => {
            let _ = selector;
            None
        }
    }
}

fn expression_identifier(expression: &Expression<'_>) -> Option<IdentifierText> {
    match expression.unparenthesized() {
        Expression::Identifier(identifier) => Some(IdentifierText {
            value: identifier_string(Some(identifier.value()))?,
            fully_qualified: matches!(identifier, Identifier::FullyQualified(_)),
        }),
        _ => None,
    }
}

struct IdentifierText {
    value: String,
    fully_qualified: bool,
}

fn resolve_identifier(
    name: &IdentifierText,
    imports: &HashMap<String, String>,
    namespace: &str,
) -> String {
    let trimmed = trim_leading_slash(&name.value);
    if name.fully_qualified {
        return trimmed;
    }
    let first = trimmed
        .split('\\')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(full) = imports.get(&first) {
        let rest = trimmed.split_once('\\').map(|(_, rest)| rest);
        return match rest {
            Some(rest) => format!("{full}\\{rest}"),
            None => full.clone(),
        };
    }
    if namespace.is_empty() || trimmed.contains('\\') {
        trimmed
    } else {
        format!("{namespace}\\{trimmed}")
    }
}

fn class_from_hint(hint: &Hint<'_>, ctx: &Context) -> Option<String> {
    match hint {
        Hint::Identifier(identifier) => {
            let name = identifier_string(Some(identifier.value()))?;
            if is_builtin_type(&name) {
                return None;
            }
            Some(resolve_identifier(
                &IdentifierText {
                    value: name,
                    fully_qualified: matches!(identifier, Identifier::FullyQualified(_)),
                },
                &ctx.imports,
                &ctx.namespace,
            ))
        }
        Hint::Nullable(nullable) => class_from_hint(nullable.hint, ctx),
        Hint::Parenthesized(parenthesized) => class_from_hint(parenthesized.hint, ctx),
        Hint::Union(union) => {
            class_from_hint(union.left, ctx).or_else(|| class_from_hint(union.right, ctx))
        }
        Hint::Intersection(intersection) => class_from_hint(intersection.left, ctx),
        _ => None,
    }
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "int"
            | "integer"
            | "float"
            | "double"
            | "bool"
            | "boolean"
            | "string"
            | "array"
            | "object"
            | "mixed"
            | "callable"
            | "iterable"
            | "void"
            | "never"
            | "null"
            | "false"
            | "true"
            | "self"
            | "static"
            | "parent"
    )
}

fn route_label(lists: &Sequence<'_, AttributeList<'_>>) -> Option<String> {
    for list in lists.iter() {
        for attribute in list.attributes.iter() {
            let name = String::from_utf8_lossy(attribute.name.value()).to_string();
            if tail_key(&name).to_ascii_lowercase() != "route" {
                continue;
            }
            let Some(arguments) = &attribute.argument_list else {
                continue;
            };
            let mut path = None;
            let mut methods = Vec::new();
            for argument in arguments.arguments.iter() {
                match argument {
                    PartialArgument::Positional(positional) => {
                        if path.is_none() {
                            path = literal_string(positional.value);
                        } else if methods.is_empty() {
                            methods = string_list(positional.value);
                        }
                    }
                    PartialArgument::Named(named) => {
                        let key = String::from_utf8_lossy(named.name.value).to_ascii_lowercase();
                        if key == "methods" {
                            methods = string_list(named.value);
                        } else if key == "path" && path.is_none() {
                            path = literal_string(named.value);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(path) = path {
                let methods = methods
                    .into_iter()
                    .map(|method| method.to_ascii_uppercase())
                    .collect::<Vec<_>>();
                let method = if methods.is_empty() {
                    "ANY".to_string()
                } else {
                    methods.join("|")
                };
                return Some(format!("{method} {path}"));
            }
        }
    }
    None
}

fn string_list(expression: &Expression<'_>) -> Vec<String> {
    match expression.unparenthesized() {
        Expression::Array(array) => array
            .elements
            .iter()
            .filter_map(|element| match element {
                ArrayElement::Value(value) => literal_string(value.value),
                _ => None,
            })
            .collect(),
        Expression::LegacyArray(array) => array
            .elements
            .iter()
            .filter_map(|element| match element {
                ArrayElement::Value(value) => literal_string(value.value),
                _ => None,
            })
            .collect(),
        _ => literal_string(expression).into_iter().collect(),
    }
}

fn selector_text(selector: &ClassLikeMemberSelector<'_>) -> Option<String> {
    match selector {
        ClassLikeMemberSelector::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.value).to_string())
        }
        _ => None,
    }
}

fn variable_name<'a>(variable: &'a Variable<'a>) -> Option<&'a [u8]> {
    match variable {
        Variable::Direct(direct) => Some(direct.name),
        _ => None,
    }
}

fn identifier_string(value: Option<&[u8]>) -> Option<String> {
    Some(
        String::from_utf8_lossy(value?)
            .trim_start_matches('\\')
            .to_string(),
    )
}

fn class_name(ctx: &Context, _prefix: &[u8], name: &[u8]) -> String {
    qualified(ctx, &String::from_utf8_lossy(name))
}

fn qualified(ctx: &Context, name: &str) -> String {
    if ctx.namespace.is_empty() {
        name.to_string()
    } else {
        format!("{}\\{name}", ctx.namespace)
    }
}

fn trim_leading_slash(value: &str) -> String {
    value.trim_start_matches('\\').to_string()
}

fn same_path(left: &Path, right: &Path) -> bool {
    left == right
        || matches!(
            (std::fs::canonicalize(left), std::fs::canonicalize(right)),
            (Ok(left), Ok(right)) if left == right
        )
}

fn normalize(value: &str) -> String {
    trim_leading_slash(value).to_ascii_lowercase()
}

fn tail_key(value: &str) -> String {
    let trimmed = trim_leading_slash(value);
    let method = trimmed.rsplit("::").next().unwrap_or(&trimmed);
    method
        .rsplit('\\')
        .next()
        .unwrap_or(method)
        .to_ascii_lowercase()
}

fn dedupe(sites: Vec<CallSite>) -> Vec<CallSite> {
    let mut seen = HashSet::new();
    sites
        .into_iter()
        .filter(|site| {
            seen.insert((
                normalize(&site.caller),
                site.file.clone(),
                site.line,
                site.route.clone(),
            ))
        })
        .collect()
}

fn format_chain(route: &str, chain: &[CallSite], target: &str) -> String {
    let mut parts = vec![route.to_string()];
    for site in chain.iter().rev() {
        let file = site.file.to_string_lossy();
        parts.push(format!("{}() at {}:{}", site.caller, file, site.line));
    }
    parts.push(target.to_string());
    parts.join(" → ")
}

#[cfg(test)]
mod tests {
    use mago_allocator::prelude::LocalArena;

    use super::*;

    #[test]
    fn infers_a_route_to_controller_to_service_chain() {
        let arena = LocalArena::new();
        let file = File::new(
            &arena,
            PathBuf::from("src/Controller/CheckoutController.php"),
            r#"<?php

namespace App\Controller;

use App\Service\DiscountCalculator;
use Symfony\Component\Routing\Attribute\Route;

final class CheckoutController
{
    public function __construct(private DiscountCalculator $calculator)
    {
    }

    #[Route('/checkout', methods: ['POST'])]
    public function submit(): void
    {
        $this->calculator->discountedTotal(100, 0);
    }
}
"#
            .to_string(),
        );
        let service = File::new(
            &arena,
            PathBuf::from("src/Service/DiscountCalculator.php"),
            r#"<?php

namespace App\Service;

final class DiscountCalculator
{
    public function discountedTotal(int $total, int $divisor): float
    {
        return $total / $divisor;
    }
}
"#
            .to_string(),
        );

        let index = CallChainIndex::default();
        index.index_file(&file);
        index.index_file(&service);

        let chain = index.chain_for(
            "App\\Service\\DiscountCalculator::discountedTotal",
            "App\\Service\\DiscountCalculator::discountedTotal() at src/Service/DiscountCalculator.php:8",
        );
        assert_eq!(chain.len(), 1, "{chain:?}");
        assert!(
            chain[0].starts_with("POST /checkout → App\\Controller\\CheckoutController::submit()")
        );
        assert!(chain[0].contains("discountedTotal() at src/Service/DiscountCalculator.php:8"));

        let by_location =
            index.chain_for_location(Path::new("src/Service/DiscountCalculator.php"), 9);
        assert_eq!(by_location.len(), 1, "{by_location:?}");
        assert!(by_location[0].starts_with(
            "POST /checkout → App\\Controller\\CheckoutController::submit() at src/Controller/CheckoutController.php:"
        ));
        assert!(by_location
            [0]
            .contains("App\\Service\\DiscountCalculator::discountedTotal() at src/Service/DiscountCalculator.php:9"));
    }
}
