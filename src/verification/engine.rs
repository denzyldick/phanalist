use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::{Add, Mul, Sub};
use std::sync::Mutex;

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;
use mago_syntax::walker::Walker;
use z3::ast::{Bool, Float, Int, RoundingMode};
use z3::{Params, SatResult, Solver};

use crate::file::File;
use crate::results::Message;

use super::class::FindingClass;
use super::contracts::{self, Docblock};
use super::summary::{self, ConstantValue, FunctionSummary};
use super::value::{ArrayKey, ResourceState, Value};
use super::Settings;

pub struct Finding {
    pub class: FindingClass,
    pub message: Message,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
enum ParameterDomain {
    Finite(Vec<ConstantValue>),
    IntRange { min: i64, max: i64 },
}

#[derive(Clone)]
struct State {
    env: HashMap<String, Value>,
    path: Bool,
    reachable: bool,
    loop_exits: Bool,
    loop_continues: Bool,
    switch_exits: Bool,
    modified: HashSet<String>,
    array_values: HashMap<String, Vec<(Value, Value)>>,
}

impl State {
    fn new() -> Self {
        Self {
            env: HashMap::new(),
            path: Bool::from_bool(true),
            reachable: true,
            loop_exits: Bool::from_bool(false),
            loop_continues: Bool::from_bool(false),
            switch_exits: Bool::from_bool(false),
            modified: HashSet::new(),
            array_values: HashMap::new(),
        }
    }

    fn assign(&mut self, name: String, value: Value) {
        self.env.insert(name.clone(), value);
        self.modified.insert(name.clone());
        self.array_values.remove(&name);
    }

    fn havoc(&mut self, name: &str) {
        self.env.insert(name.to_string(), Value::Unknown);
        self.modified.insert(name.to_string());
        self.array_values.remove(name);
    }
}

struct Analyzer<'a, 'arena> {
    #[allow(dead_code)]
    file: &'a File<'arena>,
    settings: &'a Settings,
    classes: &'a [FindingClass],
    findings: Vec<Finding>,
    seen: HashSet<(u32, u32, FindingClass, &'static str)>,
    return_hint: Option<&'arena Hint<'arena>>,
    strict_types: bool,
    finding_limit_start: usize,
    suppress_access_checks: bool,
    summaries: &'a HashMap<String, FunctionSummary>,
    parents: &'a HashMap<String, String>,
    imports: HashMap<String, HashMap<String, String>>,
    function_imports: HashMap<String, HashMap<String, String>>,
    namespace: Option<String>,
    class: Option<String>,
    switch_depth: usize,
    docblocks: Vec<Trivia<'arena>>,
    doc_return: Option<String>,
    doc_templates: Vec<String>,
    method_domains: HashMap<String, Vec<Option<ParameterDomain>>>,
    in_generator: bool,
    next_unverified_numeric: u64,
    next_fresh: u64,
    next_resource: u64,
}

pub fn analyze_file_with_summaries(
    file: &File<'_>,
    settings: &Settings,
    summaries: &HashMap<String, FunctionSummary>,
    parents: &HashMap<String, String>,
    classes: &[FindingClass],
) -> Vec<Finding> {
    let Some(program) = file.ast else {
        return Vec::new();
    };

    let mut analyzer = Analyzer {
        file,
        settings,
        classes,
        findings: Vec::new(),
        seen: HashSet::new(),
        return_hint: None,
        strict_types: file_declares_strict_types(program),
        finding_limit_start: 0,
        suppress_access_checks: false,
        summaries,
        parents,
        imports: HashMap::new(),
        function_imports: HashMap::new(),
        namespace: file
            .namespace
            .as_ref()
            .map(|namespace| namespace.to_ascii_lowercase()),
        class: None,
        switch_depth: 0,
        docblocks: program
            .trivia
            .iter()
            .filter(|trivia| trivia.kind.is_docblock())
            .copied()
            .collect(),
        doc_return: None,
        doc_templates: Vec::new(),
        method_domains: HashMap::new(),
        in_generator: false,
        next_unverified_numeric: 0,
        next_fresh: 0,
        next_resource: 0,
    };

    let mut state = State::new();
    for statement in program.statements.iter() {
        analyzer.analyze_statement(statement, &mut state);
    }

    if !classes.is_empty() {
        analyzer
            .findings
            .retain(|finding| classes.contains(&finding.class));
    }

    analyzer.findings
}

impl<'a, 'arena> Analyzer<'a, 'arena> {
    fn analyze_statement(&mut self, statement: &'arena Statement<'arena>, state: &mut State) {
        if !state.reachable && !is_declaration(statement) {
            if self.settings.check_unreachable {
                self.push_finding(
                    FindingClass::Unreachable,
                    "unreachable",
                    "This statement is unreachable.",
                    statement.span(),
                    "?",
                );
            }
            return;
        }

        match statement {
            Statement::Block(block) => self.analyze_statements(block.statements.as_slice(), state),
            Statement::Use(use_) => {
                let namespace = self.namespace.clone().unwrap_or_default();
                summary::add_imports(
                    &use_.items,
                    self.imports.entry(namespace.clone()).or_default(),
                );
                summary::add_function_imports(
                    &use_.items,
                    self.function_imports.entry(namespace).or_default(),
                );
            }
            Statement::Namespace(namespace) => {
                let previous = self.namespace.take();
                self.namespace = namespace.name.as_ref().map(|name| {
                    String::from_utf8_lossy(name.value())
                        .trim_start_matches('\\')
                        .to_ascii_lowercase()
                });
                self.analyze_statements(namespace.statements().as_slice(), state);
                self.namespace = previous;
            }
            Statement::Function(function) => {
                let previous_class = self.class.take();
                self.analyze_function(
                    &String::from_utf8_lossy(function.name.value),
                    &function.parameter_list,
                    function.return_type_hint.as_ref(),
                    &function.body,
                    function.span(),
                );
                self.class = previous_class;
            }
            Statement::Class(class) => {
                self.analyze_class(&String::from_utf8_lossy(class.name.value), &class.members)
            }
            Statement::Interface(interface) => self.analyze_class(
                &String::from_utf8_lossy(interface.name.value),
                &interface.members,
            ),
            Statement::Trait(trait_) => {
                self.analyze_class(&String::from_utf8_lossy(trait_.name.value), &trait_.members)
            }
            Statement::Enum(enum_) => {
                self.analyze_class(&String::from_utf8_lossy(enum_.name.value), &enum_.members)
            }
            Statement::If(if_statement) => self.analyze_if(if_statement, state),
            Statement::While(while_statement) => {
                let body = while_statement.body.statements();
                self.analyze_loop(Some(while_statement.condition), body, None, state);
            }
            Statement::DoWhile(do_while) => self.analyze_do_while(do_while, state),
            Statement::For(for_statement) => {
                for expression in for_statement.initializations.iter() {
                    self.eval_expr(expression, state);
                }
                let condition = for_statement.conditions.iter().last().copied();
                let body = for_statement.body.statements();
                self.analyze_loop(
                    condition,
                    body,
                    Some(for_statement.increments.as_slice()),
                    state,
                );
            }
            Statement::Switch(switch) => self.analyze_switch(switch, state),
            Statement::Foreach(foreach) => self.analyze_foreach(foreach, state),
            Statement::Try(try_statement) => self.analyze_try(try_statement, state),
            Statement::Expression(expression) => {
                let docblock = self.docblock_before(expression.span()).unwrap_or_default();
                let assignment = match expression.expression.unparenthesized() {
                    Expression::Assignment(assignment) => direct_variable_name(assignment.lhs),
                    _ => None,
                };
                self.apply_docblock_vars(&docblock, state, assignment.as_deref());
                self.eval_expr(expression.expression, state);
                self.apply_docblock_vars(&docblock, state, assignment.as_deref());
            }
            Statement::Echo(echo) => {
                for value in echo.values.iter() {
                    self.eval_expr(value, state);
                }
            }
            Statement::EchoTag(echo) => {
                for value in echo.values.iter() {
                    self.eval_expr(value, state);
                }
            }
            Statement::Return(return_statement) => {
                let docblock = self
                    .docblock_before(return_statement.span())
                    .unwrap_or_default();
                self.apply_docblock_vars(&docblock, state, None);
                let value = return_statement
                    .value
                    .map(|value| self.eval_expr(value, state));
                if self.settings.check_return_contracts {
                    self.check_return_contract(value.as_ref(), return_statement.span());
                }
                state.path = Bool::from_bool(false);
                state.reachable = false;
            }
            Statement::Break(break_statement) => {
                let level = control_flow_level(break_statement.level);
                if level == 1 && self.switch_depth > 0 {
                    state.switch_exits = Bool::or(&[&state.switch_exits, &state.path]);
                } else {
                    state.loop_exits = Bool::or(&[&state.loop_exits, &state.path]);
                }
                state.path = Bool::from_bool(false);
                state.reachable = false;
            }
            Statement::Continue(continue_statement) => {
                let level = control_flow_level(continue_statement.level);
                if level == 1 && self.switch_depth > 0 {
                    state.switch_exits = Bool::or(&[&state.switch_exits, &state.path]);
                } else {
                    state.loop_continues = Bool::or(&[&state.loop_continues, &state.path]);
                }
                state.path = Bool::from_bool(false);
                state.reachable = false;
            }
            Statement::Declare(declare) => {
                if let DeclareBody::Statement(statement) = &declare.body {
                    self.analyze_statement(statement, state);
                } else if let DeclareBody::ColonDelimited(body) = &declare.body {
                    self.analyze_statements(body.statements.as_slice(), state);
                }
            }
            Statement::Global(global) => {
                for variable in global.variables.iter() {
                    if let Variable::Direct(variable) = variable {
                        state.assign(
                            String::from_utf8_lossy(variable.name).to_string(),
                            Value::Unknown,
                        );
                    }
                }
            }
            Statement::Static(static_statement) => {
                for item in static_statement.items.iter() {
                    state.assign(
                        String::from_utf8_lossy(item.variable().name).to_string(),
                        Value::Unknown,
                    );
                }
            }
            Statement::Unset(unset) => {
                for value in unset.values.iter() {
                    if let Some(name) = root_variable_name(value) {
                        state.env.remove(&name);
                        state.array_values.remove(&name);
                    }
                }
            }
            _ => {}
        }
    }

    fn analyze_statements(&mut self, statements: &'arena [Statement<'arena>], state: &mut State) {
        for statement in statements {
            self.analyze_statement(statement, state);
        }
    }

    fn analyze_class(
        &mut self,
        name: &str,
        members: &'arena Sequence<'arena, ClassLikeMember<'arena>>,
    ) {
        let previous = self.class.take();
        let previous_domains = std::mem::take(&mut self.method_domains);
        let class = self.qualified_name(&name.to_ascii_lowercase());
        self.method_domains = self.static_method_domains(&class, members);
        self.class = Some(class);
        self.analyze_members(members);
        self.class = previous;
        self.method_domains = previous_domains;
    }

    fn static_method_domains(
        &self,
        class: &str,
        members: &'arena Sequence<'arena, ClassLikeMember<'arena>>,
    ) -> HashMap<String, Vec<Option<ParameterDomain>>> {
        if members
            .iter()
            .any(|member| matches!(member, ClassLikeMember::TraitUse(_)))
        {
            return HashMap::new();
        }

        let no_subclass = !self.parents.values().any(|parent| parent == class);
        let mut parameters: HashMap<String, Vec<String>> = HashMap::new();
        for member in members.iter() {
            let ClassLikeMember::Method(method) = member else {
                continue;
            };
            let callable = method.modifiers.iter().any(Modifier::is_private)
                || (no_subclass && method.modifiers.iter().any(Modifier::is_protected));
            if !matches!(method.body, MethodBody::Concrete(_))
                || !callable
                || method
                    .parameter_list
                    .parameters
                    .iter()
                    .any(|parameter| parameter.ellipsis.is_some() || parameter.ampersand.is_some())
            {
                continue;
            }
            let name = String::from_utf8_lossy(method.name.value).to_ascii_lowercase();
            let names = method
                .parameter_list
                .parameters
                .iter()
                .map(|parameter| {
                    String::from_utf8_lossy(parameter.variable.name)
                        .trim_start_matches('$')
                        .to_string()
                })
                .collect();
            parameters.insert(name, names);
        }
        if parameters.is_empty() {
            return HashMap::new();
        }

        let namespace = self.namespace.clone().unwrap_or_default();
        let imports = self.imports.get(&namespace).cloned().unwrap_or_default();
        let mut domains: HashMap<String, Vec<Option<ParameterDomain>>> = parameters
            .iter()
            .map(|(name, parameters)| (name.clone(), vec![None; parameters.len()]))
            .collect();

        for _ in 0..16 {
            let calls = Mutex::new(HashMap::new());
            let invalid = Mutex::new(HashSet::new());
            let inner_scope = Mutex::new(0usize);
            for member in members.iter() {
                let ClassLikeMember::Method(method) = member else {
                    continue;
                };
                let MethodBody::Concrete(block) = &method.body else {
                    continue;
                };
                let method_name = String::from_utf8_lossy(method.name.value).to_ascii_lowercase();
                let caller_parameters = parameters
                    .get(&method_name)
                    .cloned()
                    .or_else(|| {
                        let names = method
                            .parameter_list
                            .parameters
                            .iter()
                            .map(|parameter| {
                                String::from_utf8_lossy(parameter.variable.name)
                                    .trim_start_matches('$')
                                    .to_string()
                            })
                            .collect::<Vec<_>>();
                        (!names.is_empty()).then_some(names)
                    })
                    .unwrap_or_default();
                let caller_domains = domains
                    .get(&method_name)
                    .cloned()
                    .unwrap_or_else(|| vec![None; caller_parameters.len()]);
                let collector = PrivateStaticCallCollector {
                    class,
                    namespace: self.namespace.as_deref(),
                    imports: &imports,
                    parameters: &parameters,
                    allow_static_binding: no_subclass,
                    caller_parameters: &caller_parameters,
                    caller_domains: &caller_domains,
                    domains: &domains,
                    calls: &calls,
                    invalid: &invalid,
                    inner_scope: &inner_scope,
                };
                collector.walk_block(block, &mut ());
            }

            let invalid = invalid.into_inner().unwrap_or_default();
            let collected = calls.into_inner().unwrap_or_default();
            let mut changed = false;
            for (name, calls) in collected {
                if invalid.contains(&name) {
                    continue;
                }
                let Some(arity) = parameters.get(&name).map(Vec::len) else {
                    continue;
                };
                let inferred = infer_parameter_domains(&calls, arity);
                let Some(current) = domains.get_mut(&name) else {
                    continue;
                };
                for (index, domain) in inferred.into_iter().enumerate() {
                    let Some(domain) = domain else {
                        continue;
                    };
                    let merged = match current[index].as_ref() {
                        Some(current) => merge_domains(current, &domain),
                        None => Some(domain),
                    };
                    if merged != current[index] {
                        current[index] = merged;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        domains.retain(|_, domains| domains.iter().any(Option::is_some));
        domains
    }

    fn analyze_members(&mut self, members: &'arena Sequence<'arena, ClassLikeMember<'arena>>) {
        for member in members.iter() {
            if let ClassLikeMember::Method(method) = member {
                if let MethodBody::Concrete(block) = &method.body {
                    self.analyze_function(
                        &String::from_utf8_lossy(method.name.value),
                        &method.parameter_list,
                        method.return_type_hint.as_ref(),
                        block,
                        method.span(),
                    );
                }
            }
        }
    }

    fn qualified_name(&self, name: &str) -> String {
        match &self.namespace {
            Some(namespace) if !namespace.is_empty() => format!("{namespace}\\{name}"),
            _ => name.to_string(),
        }
    }

    fn analyze_function(
        &mut self,
        name: &str,
        parameters: &'arena FunctionLikeParameterList<'arena>,
        return_hint: Option<&'arena FunctionLikeReturnTypeHint<'arena>>,
        body: &'arena Block<'arena>,
        span: Span,
    ) {
        let previous_hint = self.return_hint;
        let previous_doc_return = self.doc_return.take();
        let previous_doc_templates = std::mem::take(&mut self.doc_templates);
        let previous_limit = self.finding_limit_start;
        let previous_generator = self.in_generator;
        self.return_hint = return_hint.map(|hint| &hint.hint);
        self.in_generator = body_contains_yield(body);
        self.finding_limit_start = self.findings.len();

        let docblock = self.docblock_before(span).unwrap_or_default();
        self.doc_return = docblock.return_type.clone();
        self.doc_templates = docblock.templates.clone();

        let mut state = State::new();
        let method_domains = self.method_domains.get(&name.to_ascii_lowercase());
        for (index, parameter) in parameters.parameters.iter().enumerate() {
            let parameter_name = String::from_utf8_lossy(parameter.variable.name).to_string();
            let doc_type = if self.settings.check_types {
                docblock
                    .params
                    .get(parameter_name.strip_prefix('$').unwrap_or(&parameter_name))
                    .map(String::as_str)
            } else {
                None
            };
            let inferred = method_domains
                .and_then(|domains| domains.get(index))
                .and_then(Option::as_ref)
                .and_then(|domain| domain_parameter_value(name, &parameter_name, domain));
            let (value, constant_constraint) = inferred.unwrap_or_else(|| {
                (
                    parameter_value(parameter.hint.as_ref(), doc_type, name, &parameter_name),
                    None,
                )
            });
            let value = if matches!(value, Value::Unknown) {
                self.parameter_object_value(parameter.hint.as_ref())
                    .unwrap_or(value)
            } else {
                value
            };
            if let Some(constraint) = constant_constraint {
                state.path = path_and(&state.path, &constraint);
            }
            if let Some(doc_type) = doc_type {
                if let Some(constraint) = contracts::constraint(&value, doc_type) {
                    state.path = path_and(&state.path, &constraint);
                }
            }
            state.env.insert(parameter_name, value);
        }

        self.analyze_statements(body.statements.as_slice(), &mut state);

        self.return_hint = previous_hint;
        self.doc_return = previous_doc_return;
        self.doc_templates = previous_doc_templates;
        self.finding_limit_start = previous_limit;
        self.in_generator = previous_generator;
    }

    fn analyze_closure(
        &mut self,
        closure: &'arena Closure<'arena>,
        state: &State,
    ) -> HashSet<String> {
        let docblock = self.docblock_before(closure.span()).unwrap_or_default();
        let previous_hint = self.return_hint;
        let previous_doc_return =
            std::mem::replace(&mut self.doc_return, docblock.return_type.clone());
        let previous_doc_templates =
            std::mem::replace(&mut self.doc_templates, docblock.templates.clone());
        let previous_limit = self.finding_limit_start;
        let previous_generator = self.in_generator;
        self.return_hint = closure.return_type_hint.as_ref().map(|hint| &hint.hint);
        self.in_generator = body_contains_yield(&closure.body);
        self.finding_limit_start = self.findings.len();

        let mut body_state = state.clone();
        if let Some(use_clause) = &closure.use_clause {
            for variable in use_clause.variables.iter() {
                let name = String::from_utf8_lossy(variable.variable.name).to_string();
                let value = state.env.get(&name).cloned().unwrap_or(Value::Unknown);
                body_state.env.insert(name.clone(), value);
                if variable.ampersand.is_some() {
                    body_state.assign(name, Value::Unknown);
                }
            }
        }
        for parameter in closure.parameter_list.parameters.iter() {
            let name = String::from_utf8_lossy(parameter.variable.name).to_string();
            let doc_type = if self.settings.check_types {
                docblock
                    .params
                    .get(name.strip_prefix('$').unwrap_or(&name))
                    .map(String::as_str)
            } else {
                None
            };
            let value = parameter_value(parameter.hint.as_ref(), doc_type, "closure", &name);
            body_state.env.insert(name, value);
        }

        body_state.modified.clear();
        self.analyze_statements(closure.body.statements.as_slice(), &mut body_state);
        let modified = body_state.modified.clone();

        self.return_hint = previous_hint;
        self.doc_return = previous_doc_return;
        self.doc_templates = previous_doc_templates;
        self.finding_limit_start = previous_limit;
        self.in_generator = previous_generator;

        modified
    }

    fn analyze_arrow_function(&mut self, arrow: &'arena ArrowFunction<'arena>, state: &State) {
        let docblock = self.docblock_before(arrow.span()).unwrap_or_default();
        let previous_hint = self.return_hint;
        let previous_doc_return =
            std::mem::replace(&mut self.doc_return, docblock.return_type.clone());
        let previous_doc_templates =
            std::mem::replace(&mut self.doc_templates, docblock.templates.clone());
        let previous_limit = self.finding_limit_start;
        let previous_generator = self.in_generator;
        self.return_hint = arrow.return_type_hint.as_ref().map(|hint| &hint.hint);
        self.in_generator = false;
        self.finding_limit_start = self.findings.len();

        let mut body_state = state.clone();
        for parameter in arrow.parameter_list.parameters.iter() {
            let name = String::from_utf8_lossy(parameter.variable.name).to_string();
            let doc_type = if self.settings.check_types {
                docblock
                    .params
                    .get(name.strip_prefix('$').unwrap_or(&name))
                    .map(String::as_str)
            } else {
                None
            };
            let value = parameter_value(parameter.hint.as_ref(), doc_type, "arrow function", &name);
            body_state.env.insert(name, value);
        }

        let value = self.eval_expr(arrow.expression, &mut body_state);
        if self.settings.check_return_contracts {
            self.check_return_contract(Some(&value), arrow.expression.span());
        }

        self.return_hint = previous_hint;
        self.doc_return = previous_doc_return;
        self.doc_templates = previous_doc_templates;
        self.finding_limit_start = previous_limit;
        self.in_generator = previous_generator;
    }

    fn apply_docblock_vars(
        &self,
        docblock: &Docblock,
        state: &mut State,
        assignment: Option<&str>,
    ) {
        if !self.settings.check_types {
            return;
        }
        for (name, ty) in &docblock.vars {
            self.apply_docblock_var(name, ty, state);
        }
        if let (Some(name), Some(ty)) = (assignment, docblock.var.as_deref()) {
            self.apply_docblock_var(name, ty, state);
        }
    }

    fn apply_docblock_var(&self, name: &str, ty: &str, state: &mut State) {
        let value = state.env.get(name).cloned().unwrap_or(Value::Unknown);
        let value = match value {
            Value::Unknown => parameter_value(
                None,
                Some(ty),
                "docblock",
                name.strip_prefix('$').unwrap_or(name),
            ),
            value => value,
        };
        if let Some(constraint) = contracts::constraint(&value, ty) {
            state.path = path_and(&state.path, &constraint);
        }
        state.env.insert(name.to_string(), value);
    }

    fn docblock_before(&self, span: Span) -> Option<Docblock> {
        let program = self.file.ast?;
        let mut best: Option<&Trivia<'arena>> = None;
        for trivia in &self.docblocks {
            if trivia.span.end.offset > span.start.offset {
                continue;
            }
            if best.is_none_or(|best| trivia.span.end.offset > best.span.end.offset) {
                best = Some(trivia);
            }
        }
        let best = best?;
        let start = best.span.end.offset as usize;
        let end = span.start.offset as usize;
        let gap = program.source_text.get(start..end)?;
        if !gap.iter().all(u8::is_ascii_whitespace) {
            return None;
        }
        let docblock = contracts::parse_docblock(best.value);
        (!docblock.is_empty()).then_some(docblock)
    }

    fn analyze_try(&mut self, try_statement: &'arena Try<'arena>, state: &mut State) {
        let entry = state.clone();
        let mut try_state = entry.clone();
        self.analyze_statements(try_statement.block.statements.as_slice(), &mut try_state);

        let mut branch_states = vec![try_state];
        for catch in try_statement.catch_clauses.iter() {
            let mut catch_state = entry.clone();
            if let Some(variable) = &catch.variable {
                catch_state.assign(
                    String::from_utf8_lossy(variable.name).to_string(),
                    Value::Unknown,
                );
            }
            self.analyze_statements(catch.block.statements.as_slice(), &mut catch_state);
            branch_states.push(catch_state);
        }

        let mut merged = entry.clone();
        let mut paths = Vec::new();
        let mut modified = HashSet::new();
        for branch in &branch_states {
            paths.push(branch.path.clone());
            modified.extend(branch.modified.iter().cloned());
        }
        for name in modified {
            merged.havoc(&name);
        }
        merged.path = path_or_many(&paths);
        merged.loop_exits = path_or_many(
            &branch_states
                .iter()
                .map(|branch| branch.loop_exits.clone())
                .collect::<Vec<_>>(),
        );
        merged.loop_continues = path_or_many(
            &branch_states
                .iter()
                .map(|branch| branch.loop_continues.clone())
                .collect::<Vec<_>>(),
        );
        merged.switch_exits = path_or_many(
            &branch_states
                .iter()
                .map(|branch| branch.switch_exits.clone())
                .collect::<Vec<_>>(),
        );
        merged.reachable = branch_states.iter().any(|branch| branch.reachable);

        if let Some(finally) = &try_statement.finally_clause {
            let mut finally_state = merged.clone();
            finally_state.reachable = entry.reachable || merged.reachable;
            if !path_may_be_true(&finally_state.path) {
                finally_state.path = entry.path.clone();
            }
            self.analyze_statements(finally.block.statements.as_slice(), &mut finally_state);

            for name in &finally_state.modified {
                merged.havoc(name);
            }
            if !finally_state.reachable {
                merged.reachable = false;
                merged.path = Bool::from_bool(false);
            }
        }

        *state = merged;
    }

    fn analyze_if(&mut self, if_statement: &'arena If<'arena>, state: &mut State) {
        let condition_value = self.eval_expr(if_statement.condition, state);
        let condition = condition_value.truthiness();

        let mut branches: Vec<(Option<Bool>, &'arena [Statement<'arena>])> = Vec::new();
        branches.push((condition.clone(), if_statement.body.statements()));

        let mut remaining_path = match &condition {
            Some(condition) => path_and(&state.path, &path_not(condition)),
            None => state.path.clone(),
        };

        for (else_if_condition, statements) in if_statement.body.else_if_clauses() {
            let mut condition_state = state.clone();
            condition_state.path = remaining_path.clone();
            let value = self.eval_expr(else_if_condition, &mut condition_state);
            let value = value.truthiness();
            branches.push((value.clone(), statements));
            if let Some(value) = value {
                remaining_path = path_and(&remaining_path, &path_not(&value));
            }
        }

        let else_statements = if_statement.body.else_statements();
        let mut branch_states = Vec::new();
        for (branch_condition, statements) in &branches {
            let mut branch_state = state.clone();
            if let Some(branch_condition) = branch_condition {
                branch_state.path = path_and(&state.path, branch_condition);
            }
            self.analyze_statements(statements, &mut branch_state);
            branch_states.push(branch_state);
        }

        let mut else_state = state.clone();
        else_state.path = remaining_path;
        if let Some(statements) = else_statements {
            self.analyze_statements(statements, &mut else_state);
        }
        branch_states.push(else_state);

        let mut merged = state.clone();
        let mut modified = HashSet::new();
        for branch_state in &branch_states {
            modified.extend(branch_state.modified.iter().cloned());
        }

        for name in modified {
            let value = if branch_states.len() == 2 {
                let left = branch_states[0]
                    .env
                    .get(&name)
                    .cloned()
                    .unwrap_or(Value::Unknown);
                let right = branch_states[1]
                    .env
                    .get(&name)
                    .cloned()
                    .unwrap_or(Value::Unknown);
                Value::merge(condition.as_ref(), &left, &right)
            } else {
                Value::Unknown
            };
            merged.env.insert(name.clone(), value);
            merged.modified.insert(name.clone());
            merged.array_values.remove(&name);
        }

        let mut paths = Vec::new();
        for branch_state in &branch_states {
            paths.push(branch_state.path.clone());
        }
        merged.path = path_or_many(&paths);
        merged.loop_exits = path_or_many(
            &branch_states
                .iter()
                .map(|branch| branch.loop_exits.clone())
                .collect::<Vec<_>>(),
        );
        merged.loop_continues = path_or_many(
            &branch_states
                .iter()
                .map(|branch| branch.loop_continues.clone())
                .collect::<Vec<_>>(),
        );
        merged.switch_exits = path_or_many(
            &branch_states
                .iter()
                .map(|branch| branch.switch_exits.clone())
                .collect::<Vec<_>>(),
        );
        merged.reachable = branch_states.iter().any(|branch| branch.reachable);
        *state = merged;
    }

    fn analyze_foreach(&mut self, foreach: &'arena Foreach<'arena>, state: &mut State) {
        let elements = match foreach.expression.unparenthesized() {
            Expression::Array(array) => Some(array.elements.as_slice()),
            Expression::LegacyArray(array) => Some(array.elements.as_slice()),
            _ => None,
        };

        if let Some(elements) = elements {
            if !elements
                .iter()
                .any(|element| matches!(element, ArrayElement::Variadic(_)))
            {
                if let Some(pairs) = self.eval_array_pairs(elements, state) {
                    self.analyze_foreach_pairs(pairs, foreach, state);
                }
                return;
            }
        }

        if let Some(name) = direct_variable_name(foreach.expression) {
            if let Some(pairs) = state.array_values.get(&name).cloned() {
                self.analyze_foreach_pairs(pairs, foreach, state);
                return;
            }
        }

        let iterable = self.eval_expr(foreach.expression, state);
        if let Value::ListLength(length) = iterable {
            let initial = state.clone();
            let mut body_state = state.clone();
            if let Some(root) = foreach_reference_root(foreach) {
                body_state.havoc(&root);
            }
            let tag = self.fresh_tag();
            let key = Int::new_const(format!("foreach::key::{tag}"));
            let in_bounds = Bool::and(&[&key.ge(Int::from_i64(0)), &key.lt(length.clone())]);
            body_state.path = path_and(&body_state.path, &in_bounds);
            match &foreach.target {
                ForeachTarget::Value(target) => {
                    if let Some(name) = direct_variable_name(target.value) {
                        body_state.assign(name, Value::Unknown);
                    }
                }
                ForeachTarget::KeyValue(target) => {
                    if let Some(name) = direct_variable_name(target.key) {
                        body_state.assign(name, Value::Int(key.clone()));
                    }
                    if let Some(name) = direct_variable_name(target.value) {
                        body_state.assign(name, Value::Unknown);
                    }
                }
            }
            self.analyze_statements(foreach.body.statements(), &mut body_state);
            let modified = body_state.modified.clone();
            let ran = length.gt(Int::from_i64(0));
            self.merge_loop_modified(state, &initial, &body_state, &modified, &ran);
            return;
        }

        let initial = state.clone();
        let mut body_state = state.clone();
        if let Some(root) = foreach_reference_root(foreach) {
            body_state.havoc(&root);
        }
        match &foreach.target {
            ForeachTarget::Value(target) => {
                if let Some(name) = direct_variable_name(target.value) {
                    body_state.assign(name, Value::Unknown);
                }
            }
            ForeachTarget::KeyValue(target) => {
                if let Some(name) = direct_variable_name(target.key) {
                    body_state.assign(name, Value::Unknown);
                }
                if let Some(name) = direct_variable_name(target.value) {
                    body_state.assign(name, Value::Unknown);
                }
            }
        }
        self.analyze_statements(foreach.body.statements(), &mut body_state);
        let modified = body_state.modified.clone();
        let tag = self.fresh_tag();
        let ran = Bool::new_const(format!("foreach_ran::{tag}"));
        self.merge_loop_modified(state, &initial, &body_state, &modified, &ran);
    }

    fn analyze_foreach_pairs(
        &mut self,
        pairs: Vec<(Value, Value)>,
        foreach: &'arena Foreach<'arena>,
        state: &mut State,
    ) {
        let mut modified = HashSet::new();

        for (key, value) in pairs {
            let mut body_state = state.clone();
            if let Some(root) = foreach_reference_root(foreach) {
                body_state.havoc(&root);
            }
            match &foreach.target {
                ForeachTarget::Value(target) => {
                    if let Some(name) = direct_variable_name(target.value) {
                        body_state.assign(name, value);
                    }
                }
                ForeachTarget::KeyValue(target) => {
                    if let Some(name) = direct_variable_name(target.key) {
                        body_state.assign(name, key);
                    }
                    if let Some(name) = direct_variable_name(target.value) {
                        body_state.assign(name, value);
                    }
                }
            }

            self.analyze_statements(foreach.body.statements(), &mut body_state);
            modified.extend(body_state.modified.iter().cloned());
        }

        for name in modified {
            state.havoc(&name);
        }
    }

    fn analyze_switch(&mut self, switch: &'arena Switch<'arena>, state: &mut State) {
        let subject = self.eval_expr(switch.expression, state);
        let has_default = switch.body.has_default_case();
        let cases = switch.body.cases();
        let mut conditions = Vec::with_capacity(cases.len());
        let mut known_conditions: Vec<Bool> = Vec::new();
        let mut has_unknown_condition = false;

        for case in cases {
            let condition = case.expression().map(|expression| {
                let value = self.eval_expr(expression, state);
                self.compare_identical(&subject, &value, false).truthiness()
            });
            if let Some(Some(condition)) = &condition {
                known_conditions.push(condition.clone());
            } else if !case.is_default() {
                has_unknown_condition = true;
            }
            conditions.push(condition);
        }

        let default_condition = if has_default && !has_unknown_condition {
            Some(path_not(&path_or_many(&known_conditions)))
        } else {
            None
        };

        let mut pending: Vec<Bool> = Vec::new();
        let mut pending_unknown = false;
        let mut pending_default = false;
        let mut exits: Vec<Bool> = Vec::new();
        let mut after_states: Vec<State> = Vec::new();
        let mut all_states: Vec<State> = Vec::new();
        let mut fallthrough: Option<State> = None;

        self.switch_depth += 1;
        for (case, condition) in cases.iter().zip(conditions) {
            let case_condition = condition.flatten();
            if case.is_empty() {
                if case.is_default() {
                    pending_default = true;
                } else if let Some(condition) = case_condition {
                    pending.push(condition);
                } else {
                    pending_unknown = true;
                }
                continue;
            }

            let mut triggers = std::mem::take(&mut pending);
            let mut trigger_unknown = std::mem::take(&mut pending_unknown);
            if case.is_default() {
                match default_condition.clone() {
                    Some(condition) => triggers.push(condition),
                    None => trigger_unknown = true,
                }
            } else {
                match case_condition {
                    Some(condition) => triggers.push(condition),
                    None => trigger_unknown = true,
                }
            }
            if pending_default {
                match default_condition.clone() {
                    Some(condition) => triggers.push(condition),
                    None => trigger_unknown = true,
                }
                pending_default = false;
            }

            let direct_condition = if trigger_unknown || triggers.is_empty() {
                None
            } else {
                Some(path_or_many(&triggers))
            };

            let mut case_states = Vec::new();
            let mut direct_state = state.clone();
            if let Some(condition) = &direct_condition {
                direct_state.path = path_and(&state.path, condition);
            }
            self.analyze_statements(case.statements(), &mut direct_state);
            case_states.push(direct_state);

            if let Some(mut fall_state) = fallthrough.take() {
                fall_state.switch_exits = Bool::from_bool(false);
                self.analyze_statements(case.statements(), &mut fall_state);
                case_states.push(fall_state);
            }

            let mut fall_candidates = Vec::new();
            for mut case_state in case_states {
                if !matches!(case_state.switch_exits.as_bool(), Some(false)) {
                    exits.push(case_state.switch_exits.clone());
                }
                case_state.switch_exits = Bool::from_bool(false);
                if case_state.reachable || path_may_be_true(&case_state.path) {
                    fall_candidates.push(case_state.clone());
                }
                all_states.push(case_state);
            }
            fallthrough = self
                .merge_fallthrough_states(fall_candidates.as_slice(), direct_condition.as_ref());
        }
        self.switch_depth -= 1;

        if !pending.is_empty() || pending_unknown || pending_default {
            let mut branch_state = state.clone();
            if !pending_unknown {
                let mut triggers = pending;
                if pending_default {
                    match default_condition.clone() {
                        Some(condition) => triggers.push(condition),
                        None => pending_unknown = true,
                    }
                }
                if !pending_unknown && !triggers.is_empty() {
                    branch_state.path = path_and(&state.path, &path_or_many(&triggers));
                }
            }
            after_states.push(branch_state.clone());
            all_states.push(branch_state);
        }

        if !has_default {
            let mut branch_state = state.clone();
            if !has_unknown_condition {
                let any_case = path_or_many(&known_conditions);
                branch_state.path = path_and(&state.path, &path_not(&any_case));
            }
            after_states.push(branch_state.clone());
            all_states.push(branch_state);
        }

        if let Some(fall_state) = fallthrough {
            if fall_state.reachable || path_may_be_true(&fall_state.path) {
                after_states.push(fall_state.clone());
            }
            all_states.push(fall_state);
        }

        if all_states.is_empty() {
            return;
        }

        let mut merged = state.clone();
        let mut modified = HashSet::new();
        for branch_state in &all_states {
            modified.extend(branch_state.modified.iter().cloned());
        }
        for name in modified {
            merged.havoc(&name);
        }

        let exits_reachable = exits.iter().any(path_may_be_true);
        let mut paths = exits;
        for branch_state in &after_states {
            paths.push(branch_state.path.clone());
        }
        merged.path = path_or_many(&paths);
        merged.switch_exits = Bool::from_bool(false);
        merged.loop_exits = path_or_many(
            &all_states
                .iter()
                .map(|branch| branch.loop_exits.clone())
                .collect::<Vec<_>>(),
        );
        merged.loop_continues = path_or_many(
            &all_states
                .iter()
                .map(|branch| branch.loop_continues.clone())
                .collect::<Vec<_>>(),
        );
        merged.reachable = exits_reachable
            || all_states
                .iter()
                .any(|branch| branch.reachable || path_may_be_true(&branch.path));
        *state = merged;
    }

    fn merge_fallthrough_states(
        &self,
        states: &[State],
        condition: Option<&Bool>,
    ) -> Option<State> {
        match states {
            [] => None,
            [state] => Some(state.clone()),
            [left, right] => {
                let mut merged = left.clone();
                let mut modified = HashSet::new();
                modified.extend(left.modified.iter().cloned());
                modified.extend(right.modified.iter().cloned());
                for name in modified {
                    let left_value = left.env.get(&name).cloned().unwrap_or(Value::Unknown);
                    let right_value = right.env.get(&name).cloned().unwrap_or(Value::Unknown);
                    merged.env.insert(
                        name.clone(),
                        Value::merge(condition, &left_value, &right_value),
                    );
                    merged.modified.insert(name.clone());
                    merged.array_values.remove(&name);
                }
                merged.path = path_or_many(&[left.path.clone(), right.path.clone()]);
                merged.loop_exits =
                    path_or_many(&[left.loop_exits.clone(), right.loop_exits.clone()]);
                merged.loop_continues =
                    path_or_many(&[left.loop_continues.clone(), right.loop_continues.clone()]);
                merged.switch_exits = Bool::from_bool(false);
                merged.reachable = left.reachable || right.reachable;
                Some(merged)
            }
            _ => None,
        }
    }

    fn merge_loop_modified(
        &self,
        state: &mut State,
        initial: &State,
        final_state: &State,
        modified: &HashSet<String>,
        ran: &Bool,
    ) {
        for name in modified {
            let Some(initial_value) = initial.env.get(name).cloned() else {
                state.havoc(name);
                continue;
            };
            if !matches!(&initial_value, Value::Int(value) if value.as_i64() == Some(0)) {
                state.havoc(name);
                continue;
            }
            let final_value = final_state.env.get(name).cloned().unwrap_or(Value::Unknown);
            let value = match Value::merge(Some(ran), &final_value, &initial_value) {
                Value::Unknown => Value::Unverified,
                Value::UnverifiedNumeric { value, .. } => Value::UnverifiedNumeric {
                    value,
                    loop_accumulator: true,
                },
                value => value,
            };
            state.env.insert(name.clone(), value);
            state.modified.insert(name.clone());
            state.array_values.remove(name);
        }
    }

    fn analyze_loop(
        &mut self,
        condition: Option<&'arena Expression<'arena>>,
        body: &'arena [Statement<'arena>],
        increments: Option<&'arena [&'arena Expression<'arena>]>,
        state: &mut State,
    ) -> State {
        let depth = self.loop_unroll_depth();
        let loop_id = self.fresh_tag();
        let initial = state.clone();
        let mut current = state.clone();
        let mut exit_paths = Vec::new();
        let mut reachable = false;
        let mut modified = state.modified.clone();
        let mut ran: Option<Bool> = None;

        for _ in 0..depth {
            let condition_value =
                condition.map(|condition| self.eval_expr(condition, &mut current));
            let condition = condition_value.as_ref().and_then(Value::truthiness);
            if ran.is_none() {
                ran = Some(match (&condition_value, &condition) {
                    (None, _) => Bool::from_bool(true),
                    (Some(_), Some(condition)) => condition.clone(),
                    (Some(_), None) => Bool::new_const(format!("loop_ran::{loop_id}")),
                });
            }

            let mut body_state = current.clone();
            let mut exit_state = current.clone();
            match &condition {
                Some(condition) => {
                    body_state.path = path_and(&current.path, condition);
                    exit_state.path = path_and(&current.path, &path_not(condition));
                }
                None => {
                    body_state.path = current.path.clone();
                    exit_state.path = if condition_value.is_some() {
                        current.path.clone()
                    } else {
                        Bool::from_bool(false)
                    };
                }
            }
            exit_paths.push(exit_state.path.clone());
            reachable |= exit_state.reachable && path_may_be_true(&exit_state.path);

            self.analyze_statements(body, &mut body_state);
            if let Some(increments) = increments {
                for increment in increments {
                    self.eval_expr(increment, &mut body_state);
                }
            }
            modified.extend(body_state.modified.iter().cloned());

            let break_path = body_state.loop_exits.clone();
            if !matches!(break_path.as_bool(), Some(false)) {
                exit_paths.push(break_path);
                reachable = true;
            }
            body_state.loop_exits = Bool::from_bool(false);

            let continue_path = body_state.loop_continues.clone();
            if !matches!(continue_path.as_bool(), Some(false)) {
                body_state.path = path_or_many(&[body_state.path.clone(), continue_path.clone()]);
                body_state.reachable |= path_may_be_true(&continue_path);
            }
            body_state.loop_continues = Bool::from_bool(false);

            current = body_state;
            if !current.reachable {
                break;
            }
            if condition.and_then(|condition| condition.as_bool()) == Some(false) {
                break;
            }
        }

        exit_paths.push(current.path.clone());
        reachable |= current.reachable && path_may_be_true(&current.path);

        let ran = ran.unwrap_or_else(|| Bool::from_bool(true));
        self.merge_loop_modified(state, &initial, &current, &modified, &ran);
        state.path = path_or_many(&exit_paths);
        state.reachable = reachable;

        current
    }

    fn analyze_do_while(&mut self, do_while: &'arena DoWhile<'arena>, state: &mut State) {
        let depth = self.loop_unroll_depth();
        let mut current = state.clone();
        let mut exit_paths = Vec::new();
        let mut reachable = false;
        let mut modified = state.modified.clone();

        for _ in 0..depth {
            let mut body_state = current.clone();
            self.analyze_statement(do_while.statement, &mut body_state);

            let break_path = body_state.loop_exits.clone();
            if !matches!(break_path.as_bool(), Some(false)) {
                exit_paths.push(break_path);
                reachable = true;
            }
            body_state.loop_exits = Bool::from_bool(false);

            let continue_path = body_state.loop_continues.clone();
            if !matches!(continue_path.as_bool(), Some(false)) {
                body_state.path = path_or_many(&[body_state.path.clone(), continue_path.clone()]);
                body_state.reachable |= path_may_be_true(&continue_path);
            }
            body_state.loop_continues = Bool::from_bool(false);

            let condition_value = self.eval_expr(do_while.condition, &mut body_state);
            let condition = condition_value.truthiness();
            let known_condition = !matches!(condition_value, Value::Unknown | Value::Unverified);
            modified.extend(body_state.modified.iter().cloned());

            let mut next_state = body_state.clone();
            let mut exit_state = body_state;
            match &condition {
                Some(condition) => {
                    next_state.path = path_and(&next_state.path, condition);
                    exit_state.path = path_and(&exit_state.path, &path_not(condition));
                }
                None => {
                    exit_state.path = if known_condition {
                        exit_state.path.clone()
                    } else {
                        Bool::from_bool(false)
                    };
                }
            }

            exit_paths.push(exit_state.path.clone());
            reachable |= exit_state.reachable && path_may_be_true(&exit_state.path);

            current = next_state;
            if !current.reachable
                || condition.and_then(|condition| condition.as_bool()) == Some(false)
            {
                break;
            }
        }

        exit_paths.push(current.path.clone());
        reachable |= current.reachable && path_may_be_true(&current.path);

        for name in modified {
            state.havoc(&name);
        }
        state.path = path_or_many(&exit_paths);
        state.reachable = reachable;
    }

    fn loop_unroll_depth(&self) -> usize {
        if self.settings.loop_strategy == "none" {
            return 1;
        }

        self.settings.unroll_depth.clamp(1, 64)
    }

    fn eval_expr(&mut self, expression: &'arena Expression<'arena>, state: &mut State) -> Value {
        match expression {
            Expression::Parenthesized(parenthesized) => {
                self.eval_expr(parenthesized.expression, state)
            }
            Expression::Literal(literal) => self.eval_literal(literal),
            Expression::Variable(variable) => self.eval_variable(variable, state),
            Expression::Instantiation(instantiation) => {
                let class = self.static_class_name(instantiation.class);
                if class.is_none() {
                    self.eval_expr(instantiation.class, state);
                }
                if let Some(arguments) = &instantiation.argument_list {
                    self.eval_arguments(arguments, state);
                    self.invalidate_mutated_array_arguments(arguments, state);
                }
                match class {
                    Some(class) => Value::Object(class),
                    None => Value::Unknown,
                }
            }
            Expression::Assignment(assignment) => self.eval_assignment(assignment, state),
            Expression::Binary(binary) => self.eval_binary(binary, state),
            Expression::UnaryPrefix(unary) => self.eval_unary_prefix(unary, state),
            Expression::UnaryPostfix(unary) => {
                let value = self.eval_expr(unary.operand, state);
                match unary.operator {
                    UnaryPostfixOperator::PostIncrement(_)
                    | UnaryPostfixOperator::PostDecrement(_) => {
                        if let Some(name) = direct_variable_name(unary.operand) {
                            let next = self.increment_value(
                                &value,
                                matches!(unary.operator, UnaryPostfixOperator::PostIncrement(_)),
                            );
                            state.assign(name, next);
                        }
                    }
                }
                value
            }
            Expression::Conditional(conditional) => self.eval_conditional(conditional, state),
            Expression::Call(call) => self.eval_call(call, state),
            Expression::Clone(clone) => self.eval_expr(clone.object, state),
            Expression::Closure(closure) => {
                let by_reference = closure
                    .use_clause
                    .as_ref()
                    .map(|use_clause| {
                        use_clause
                            .variables
                            .iter()
                            .filter(|variable| variable.ampersand.is_some())
                            .map(|variable| {
                                String::from_utf8_lossy(variable.variable.name).to_string()
                            })
                            .collect::<HashSet<_>>()
                    })
                    .unwrap_or_default();
                let modified = self.analyze_closure(closure, state);
                for name in by_reference.intersection(&modified) {
                    state.assign(name.clone(), Value::Unknown);
                }
                Value::Unknown
            }
            Expression::ArrowFunction(arrow) => {
                self.analyze_arrow_function(arrow, state);
                Value::Unknown
            }
            Expression::Throw(throw) => {
                self.eval_expr(throw.exception, state);
                state.path = Bool::from_bool(false);
                state.reachable = false;
                Value::Unknown
            }
            Expression::CompositeString(string) => self.eval_string(string, state),
            Expression::Array(array) => self.eval_array_elements(array.elements.as_slice(), state),
            Expression::LegacyArray(array) => {
                self.eval_array_elements(array.elements.as_slice(), state)
            }
            Expression::ArrayAccess(access) => {
                let array = self.eval_expr(access.array, state);
                let index = self.eval_expr(access.index, state);
                self.check_array_access(&array, &index, access.span(), &state.path);
                self.array_access_value(access.array, &array, &index, state)
            }
            Expression::Access(access) => match access {
                Access::Property(access) => {
                    let object = self.eval_expr(access.object, state);
                    if self.settings.check_nullability
                        && !self.suppress_access_checks
                        && matches!(object, Value::Null)
                    {
                        self.push_finding(
                            FindingClass::NullProperty,
                            "null-property-access",
                            "Property access on null.",
                            access.span(),
                            "null",
                        );
                    }
                    Value::Unknown
                }
                Access::NullSafeProperty(access) => {
                    self.eval_expr(access.object, state);
                    Value::Unknown
                }
                Access::StaticProperty(access) => {
                    self.eval_expr(access.class, state);
                    Value::Unknown
                }
                Access::ClassConstant(access) => {
                    self.eval_expr(access.class, state);
                    Value::Unknown
                }
            },
            Expression::Construct(construct) => self.eval_construct(construct, state),
            Expression::Match(match_) => self.eval_match(match_, state),
            _ => Value::Unknown,
        }
    }

    fn eval_match(&mut self, match_: &'arena Match<'arena>, state: &mut State) -> Value {
        let subject = self.eval_expr(match_.expression, state);
        let mut fallback = Value::Unknown;
        let mut fallback_reachable = false;
        let mut branches = Vec::new();

        for arm in match_.arms.iter() {
            match arm {
                MatchArm::Default(arm) => {
                    let mut fallback_state = state.clone();
                    fallback = self.eval_expr(arm.expression, &mut fallback_state);
                    fallback_reachable =
                        fallback_state.reachable && path_may_be_true(&fallback_state.path);
                    for name in &fallback_state.modified {
                        state.havoc(name);
                    }
                }
                MatchArm::Expression(arm) => {
                    let mut conditions = Vec::new();
                    let mut has_unknown_condition = false;
                    for condition in arm.conditions.iter() {
                        let value = self.eval_expr(condition, state);
                        match self.compare_identical(&subject, &value, false).truthiness() {
                            Some(condition) => conditions.push(condition),
                            None => has_unknown_condition = true,
                        }
                    }
                    let condition = if has_unknown_condition {
                        None
                    } else {
                        Some(path_or_many(&conditions))
                    };
                    let mut branch_state = state.clone();
                    if let Some(condition) = &condition {
                        branch_state.path = path_and(&state.path, condition);
                    }
                    let value = self.eval_expr(arm.expression, &mut branch_state);
                    let reachable = branch_state.reachable && path_may_be_true(&branch_state.path);
                    for name in &branch_state.modified {
                        state.havoc(name);
                    }
                    branches.push((condition, value, reachable));
                }
            }
        }

        if !fallback_reachable && branches.iter().all(|(_, _, reachable)| !reachable) {
            state.path = Bool::from_bool(false);
            state.reachable = false;
        }

        let mut result = fallback;
        for (condition, value, _) in branches.into_iter().rev() {
            result = match condition {
                Some(condition) => Value::merge(Some(&condition), &value, &result),
                None => Value::Unknown,
            };
        }
        result
    }

    fn eval_literal(&self, literal: &'arena Literal<'arena>) -> Value {
        match literal {
            Literal::Integer(integer) => integer
                .value
                .map(|value| Value::Int(Int::from_u64(value)))
                .unwrap_or(Value::Unknown),
            Literal::Float(float) => Value::Float(Float::from_f64(float.value.into_inner())),
            Literal::String(string) => Value::String(
                string
                    .value
                    .and_then(|value| std::str::from_utf8(value).ok())
                    .map(str::to_string),
            ),
            Literal::True(_) => Value::Bool(Bool::from_bool(true)),
            Literal::False(_) => Value::Bool(Bool::from_bool(false)),
            Literal::Null(_) => Value::Null,
        }
    }

    fn eval_variable(&self, variable: &'arena Variable<'arena>, state: &State) -> Value {
        match variable {
            Variable::Direct(variable) => state
                .env
                .get(&String::from_utf8_lossy(variable.name).to_string())
                .cloned()
                .unwrap_or(Value::Unknown),
            _ => Value::Unknown,
        }
    }

    fn eval_assignment(
        &mut self,
        assignment: &'arena Assignment<'arena>,
        state: &mut State,
    ) -> Value {
        let array_elements = if matches!(assignment.operator, AssignmentOperator::Assign(_)) {
            match assignment.rhs.unparenthesized() {
                Expression::Array(array) => Some(array.elements.as_slice()),
                Expression::LegacyArray(array) => Some(array.elements.as_slice()),
                _ => None,
            }
        } else {
            None
        };
        let array_pairs =
            array_elements.and_then(|elements| self.eval_array_pairs(elements, state));
        let right = match &array_pairs {
            Some(pairs) => value_from_pairs(pairs),
            None => self.eval_expr(assignment.rhs, state),
        };
        let name = direct_variable_name(assignment.lhs);

        let value = match assignment.operator {
            AssignmentOperator::Assign(_) => right,
            AssignmentOperator::Addition(_) => {
                let left = self.eval_expr(assignment.lhs, state);
                self.add_values(&left, &right)
            }
            AssignmentOperator::Subtraction(_) => {
                let left = self.eval_expr(assignment.lhs, state);
                self.sub_values(&left, &right)
            }
            AssignmentOperator::Multiplication(_) => {
                let left = self.eval_expr(assignment.lhs, state);
                self.mul_values(&left, &right)
            }
            AssignmentOperator::Division(_) => {
                if self.settings.check_division {
                    self.check_nonzero(
                        FindingClass::Division,
                        &right,
                        &state.path,
                        assignment.span(),
                        "division-by-zero",
                        "Division by zero: the divisor can be {value}.",
                    );
                }
                let left = self.eval_expr(assignment.lhs, state);
                self.divide_values(&left, &right)
            }
            AssignmentOperator::Modulo(_) => {
                if self.settings.check_modulo
                    && matches!(
                        right,
                        Value::Int(_) | Value::Unverified | Value::UnverifiedNumeric { .. }
                    )
                {
                    self.check_nonzero(
                        FindingClass::Modulo,
                        &right,
                        &state.path,
                        assignment.span(),
                        "modulo-by-zero",
                        "Modulo by zero: the divisor can be {value}.",
                    );
                }
                let left = self.eval_expr(assignment.lhs, state);
                self.mod_values(&left, &right)
            }
            _ => Value::Unknown,
        };

        let assigned_name = name.clone();
        if let Some(name) = name {
            let array_value = if matches!(assignment.operator, AssignmentOperator::Assign(_)) {
                match &array_pairs {
                    Some(pairs) => Some(pairs.clone()),
                    None => match assignment.rhs.unparenthesized() {
                        Expression::Variable(Variable::Direct(variable)) => state
                            .array_values
                            .get(&String::from_utf8_lossy(variable.name).to_string())
                            .cloned(),
                        Expression::Call(Call::Function(function))
                            if function_name(function.function).as_deref()
                                == Some("array_filter") =>
                        {
                            self.array_pairs_for_expression(assignment.rhs, state)
                        }
                        _ => None,
                    },
                }
            } else {
                None
            };
            state.assign(name.clone(), value.clone());
            if let Some(pairs) = array_value {
                state.array_values.insert(name.clone(), pairs);
            }
            if matches!(assignment.operator, AssignmentOperator::Assign(_))
                && is_fopen_expression(assignment.rhs)
            {
                let id = self.fresh_resource_id();
                state.env.insert(
                    name,
                    Value::Resource {
                        id,
                        state: ResourceState::Open,
                    },
                );
            }
        }

        if assigned_name.is_none()
            && !matches!(
                assignment.lhs.unparenthesized(),
                Expression::ArrayAccess(_) | Expression::ArrayAppend(_)
            )
        {
            if let Some(root) = root_variable_name(assignment.lhs) {
                state.array_values.remove(&root);
            }
        }
        if matches!(assignment.operator, AssignmentOperator::Assign(_)) {
            self.record_array_write(assignment.lhs, &value, state);
        }
        value
    }

    fn record_array_write(
        &mut self,
        lhs: &'arena Expression<'arena>,
        value: &Value,
        state: &mut State,
    ) {
        match lhs.unparenthesized() {
            Expression::ArrayAccess(access) => {
                let Some(name) = direct_variable_name(access.array) else {
                    if let Some(root) = root_variable_name(access.array) {
                        if matches!(
                            state.env.get(&root),
                            Some(Value::Array(_) | Value::ArrayLength(_) | Value::ListLength(_))
                        ) {
                            state.env.insert(root.clone(), Value::Array(None));
                        }
                        state.array_values.remove(&root);
                        state.modified.insert(root);
                    }
                    return;
                };
                let index = self.eval_expr(access.index, state);
                let key = array_key(&index);
                match (state.env.get(&name).cloned(), key) {
                    (Some(Value::Array(Some(mut keys))), Some(key)) => {
                        keys.insert(key.clone());
                        state.env.insert(name.clone(), Value::Array(Some(keys)));
                        if let Some(pairs) = state.array_values.get_mut(&name) {
                            if let Some(pair) = pairs
                                .iter_mut()
                                .find(|(candidate, _)| array_key(candidate).as_ref() == Some(&key))
                            {
                                pair.1 = value.clone();
                            } else {
                                pairs.push((index, value.clone()));
                            }
                        }
                    }
                    (Some(Value::Array(Some(_))), None) => {
                        state.env.insert(name.clone(), Value::Array(None));
                        state.array_values.remove(&name);
                    }
                    (Some(Value::Array(None)), _) => {
                        state.array_values.remove(&name);
                    }
                    (Some(Value::ArrayLength(_) | Value::ListLength(_)), _) => {
                        state.env.insert(name.clone(), Value::Array(None));
                        state.array_values.remove(&name);
                    }
                    _ => {
                        state.array_values.remove(&name);
                    }
                }
                state.modified.insert(name);
            }
            Expression::ArrayAppend(append) => {
                let Some(name) = direct_variable_name(append.array) else {
                    if let Some(root) = root_variable_name(append.array) {
                        if matches!(
                            state.env.get(&root),
                            Some(Value::Array(_) | Value::ArrayLength(_) | Value::ListLength(_))
                        ) {
                            state.env.insert(root.clone(), Value::Array(None));
                        }
                        state.array_values.remove(&root);
                        state.modified.insert(root);
                    }
                    return;
                };
                match state.env.get(&name).cloned() {
                    Some(Value::Array(Some(mut keys))) => {
                        let next = keys
                            .iter()
                            .filter_map(|key| match key {
                                ArrayKey::Int(index) => Some(*index),
                                ArrayKey::String(_) => None,
                            })
                            .max()
                            .map(|index| index.saturating_add(1))
                            .unwrap_or(0);
                        keys.insert(ArrayKey::Int(next));
                        state.env.insert(name.clone(), Value::Array(Some(keys)));
                        if let Some(pairs) = state.array_values.get_mut(&name) {
                            pairs.push((Value::Int(Int::from_i64(next)), value.clone()));
                        }
                    }
                    Some(Value::Array(None)) => {
                        state.array_values.remove(&name);
                    }
                    Some(Value::ArrayLength(length)) => {
                        state.env.insert(
                            name.clone(),
                            Value::ArrayLength(length.add(Int::from_i64(1))),
                        );
                        state.array_values.remove(&name);
                    }
                    Some(Value::ListLength(length)) => {
                        state.env.insert(
                            name.clone(),
                            Value::ListLength(length.add(Int::from_i64(1))),
                        );
                        state.array_values.remove(&name);
                    }
                    _ => {
                        state.array_values.remove(&name);
                    }
                }
                state.modified.insert(name);
            }
            _ => {}
        }
    }

    fn eval_and_chain(&mut self, binary: &'arena Binary<'arena>, state: &mut State) -> Value {
        let mut operands = Vec::new();
        collect_and_operands(binary.lhs, &mut operands);
        collect_and_operands(binary.rhs, &mut operands);

        let mut committed = state.clone();
        let mut conditions = Vec::new();
        let mut definite = true;
        let mut unknown = false;
        let mut false_result = false;
        let mut may_state: Option<State> = None;

        for operand in operands {
            let value = if definite {
                self.eval_expr(operand, &mut committed)
            } else {
                let branch = may_state.as_mut().expect("may state exists");
                self.eval_expr(operand, branch)
            };
            match value.truthiness() {
                Some(condition) if condition.as_bool() == Some(false) => {
                    false_result = true;
                    break;
                }
                Some(condition) => {
                    if definite {
                        if condition.as_bool() == Some(true) {
                            committed.path = path_and(&committed.path, &condition);
                        } else {
                            definite = false;
                            may_state = Some(committed.clone());
                        }
                    }
                    if let Some(branch) = may_state.as_mut() {
                        branch.path = path_and(&branch.path, &condition);
                    }
                    conditions.push(condition);
                }
                None => {
                    if definite {
                        definite = false;
                        may_state = Some(committed.clone());
                    }
                    unknown = true;
                }
            }
        }

        if let Some(branch) = may_state {
            self.merge_expression_state(&mut committed, &branch, false);
        }
        *state = committed;

        if false_result {
            Value::Bool(Bool::from_bool(false))
        } else if unknown {
            Value::Unknown
        } else if conditions.is_empty() {
            Value::Bool(Bool::from_bool(true))
        } else {
            let conditions = conditions.iter().collect::<Vec<_>>();
            Value::Bool(Bool::and(&conditions))
        }
    }

    fn eval_binary(&mut self, binary: &'arena Binary<'arena>, state: &mut State) -> Value {
        if matches!(
            binary.operator,
            BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
        ) {
            return self.eval_and_chain(binary, state);
        }

        if matches!(
            binary.operator,
            BinaryOperator::Or(_) | BinaryOperator::LowOr(_)
        ) {
            let left = self.eval_expr(binary.lhs, state);
            let Some(condition) = left.truthiness() else {
                let mut right_state = state.clone();
                self.eval_expr(binary.rhs, &mut right_state);
                self.merge_expression_state(state, &right_state, false);
                return Value::Unknown;
            };
            if condition.as_bool() == Some(true) {
                return Value::Bool(Bool::from_bool(true));
            }
            let mut right_state = state.clone();
            right_state.path = path_and(&state.path, &path_not(&condition));
            let right = self.eval_expr(binary.rhs, &mut right_state);
            self.merge_expression_state(state, &right_state, condition.as_bool() == Some(false));
            return match right.truthiness() {
                Some(right) => Value::Bool(Bool::or(&[&condition, &right])),
                None => Value::Unknown,
            };
        }

        let left = if matches!(binary.operator, BinaryOperator::NullCoalesce(_)) {
            let previous = self.suppress_access_checks;
            self.suppress_access_checks = true;
            let value = self.eval_expr(binary.lhs, state);
            self.suppress_access_checks = previous;
            value
        } else {
            self.eval_expr(binary.lhs, state)
        };

        if matches!(binary.operator, BinaryOperator::NullCoalesce(_)) {
            match null_condition(&left) {
                Some(condition) if condition.as_bool() == Some(false) => return left,
                Some(condition) => {
                    let right = self.eval_expr(binary.rhs, state);
                    return match condition.as_bool() {
                        Some(true) => right,
                        Some(false) => left,
                        None => Value::merge(Some(&condition), &right, &left),
                    };
                }
                None => {
                    let mut right_state = state.clone();
                    self.eval_expr(binary.rhs, &mut right_state);
                    self.merge_expression_state(state, &right_state, false);
                    return Value::Unknown;
                }
            }
        }

        let right = self.eval_expr(binary.rhs, state);

        match binary.operator {
            BinaryOperator::Division(_) => {
                if self.settings.check_division {
                    self.check_nonzero(
                        FindingClass::Division,
                        &right,
                        &state.path,
                        binary.span(),
                        "division-by-zero",
                        "Division by zero: the divisor can be {value}.",
                    );
                }
                self.divide_values(&left, &right)
            }
            BinaryOperator::Modulo(_) => {
                if self.settings.check_modulo
                    && matches!(
                        right,
                        Value::Int(_) | Value::Unverified | Value::UnverifiedNumeric { .. }
                    )
                {
                    self.check_nonzero(
                        FindingClass::Modulo,
                        &right,
                        &state.path,
                        binary.span(),
                        "modulo-by-zero",
                        "Modulo by zero: the divisor can be {value}.",
                    );
                }
                self.mod_values(&left, &right)
            }
            BinaryOperator::Addition(_) => self.add_values(&left, &right),
            BinaryOperator::Subtraction(_) => self.sub_values(&left, &right),
            BinaryOperator::Multiplication(_) => self.mul_values(&left, &right),
            BinaryOperator::StringConcat(_) => self.concat_values(&left, &right),
            BinaryOperator::NullCoalesce(_) => match left {
                Value::Null => right,
                Value::Unknown => Value::Unknown,
                value => value,
            },
            BinaryOperator::Equal(_)
            | BinaryOperator::NotEqual(_)
            | BinaryOperator::AngledNotEqual(_) => self.compare_equal(
                &left,
                &right,
                matches!(
                    binary.operator,
                    BinaryOperator::NotEqual(_) | BinaryOperator::AngledNotEqual(_)
                ),
            ),
            BinaryOperator::Identical(_) | BinaryOperator::NotIdentical(_) => self
                .compare_identical(
                    &left,
                    &right,
                    matches!(binary.operator, BinaryOperator::NotIdentical(_)),
                ),
            BinaryOperator::LessThan(_) => self.compare_less(&left, &right, false),
            BinaryOperator::LessThanOrEqual(_) => self.compare_less(&left, &right, true),
            BinaryOperator::GreaterThan(_) => self.compare_greater(&left, &right, false),
            BinaryOperator::GreaterThanOrEqual(_) => self.compare_greater(&left, &right, true),
            _ => Value::Unknown,
        }
    }

    fn eval_unary_prefix(
        &mut self,
        unary: &'arena UnaryPrefix<'arena>,
        state: &mut State,
    ) -> Value {
        match unary.operator {
            UnaryPrefixOperator::Not(_) => self
                .eval_expr(unary.operand, state)
                .truthiness()
                .map(|value| Value::Bool(path_not(&value)))
                .unwrap_or(Value::Unknown),
            UnaryPrefixOperator::Plus(_) => self.eval_expr(unary.operand, state),
            UnaryPrefixOperator::Negation(_) => match self.eval_expr(unary.operand, state) {
                Value::Int(value) => match value.as_i64() {
                    Some(literal) => Value::Int(Int::from_i64(literal.wrapping_neg())),
                    None => Value::Int(value.unary_minus()),
                },
                Value::Float(value) => Value::Float(value.unary_neg()),
                _ => Value::Unknown,
            },
            UnaryPrefixOperator::BitwiseNot(_) => Value::Unknown,
            UnaryPrefixOperator::Reference(_) => {
                let previous = self.suppress_access_checks;
                self.suppress_access_checks = true;
                self.eval_expr(unary.operand, state);
                self.suppress_access_checks = previous;
                if let Some(root) = root_variable_name(unary.operand) {
                    state.havoc(&root);
                }
                Value::Unknown
            }
            UnaryPrefixOperator::ErrorControl(_) => self.eval_expr(unary.operand, state),
            UnaryPrefixOperator::IntCast(..) | UnaryPrefixOperator::IntegerCast(..) => {
                match self.eval_expr(unary.operand, state) {
                    Value::Int(value) => Value::Int(value),
                    Value::Float(value) => {
                        let literal = value.as_f64();
                        if literal.is_finite()
                            && literal >= i64::MIN as f64
                            && literal <= i64::MAX as f64
                        {
                            Value::Int(Int::from_i64(literal.trunc() as i64))
                        } else {
                            Value::Unknown
                        }
                    }
                    _ => Value::Unknown,
                }
            }
            UnaryPrefixOperator::FloatCast(..)
            | UnaryPrefixOperator::DoubleCast(..)
            | UnaryPrefixOperator::RealCast(..) => {
                match self.eval_expr(unary.operand, state).to_float() {
                    Some(value) => Value::Float(value),
                    None => Value::Unknown,
                }
            }
            UnaryPrefixOperator::BoolCast(..) | UnaryPrefixOperator::BooleanCast(..) => self
                .eval_expr(unary.operand, state)
                .truthiness()
                .map(Value::Bool)
                .unwrap_or(Value::Unknown),
            UnaryPrefixOperator::PreIncrement(_) | UnaryPrefixOperator::PreDecrement(_) => {
                let value = self.eval_expr(unary.operand, state);
                let next = self.increment_value(
                    &value,
                    matches!(unary.operator, UnaryPrefixOperator::PreIncrement(_)),
                );
                if let Some(name) = direct_variable_name(unary.operand) {
                    state.assign(name, next.clone());
                }
                next
            }
            _ => {
                self.eval_expr(unary.operand, state);
                Value::Unknown
            }
        }
    }

    fn eval_conditional(
        &mut self,
        conditional: &'arena Conditional<'arena>,
        state: &mut State,
    ) -> Value {
        let condition_value = self.eval_expr(conditional.condition, state);
        let condition = condition_value.truthiness();

        let Some(condition) = condition else {
            let mut then_state = state.clone();
            if let Some(value) = conditional.then {
                self.eval_expr(value, &mut then_state);
            }
            let mut else_state = state.clone();
            self.eval_expr(conditional.r#else, &mut else_state);
            self.merge_expression_state(state, &then_state, false);
            self.merge_expression_state(state, &else_state, false);
            if !then_state.reachable && !else_state.reachable {
                state.path = Bool::from_bool(false);
                state.reachable = false;
            }
            return Value::Unknown;
        };

        let mut then_state = state.clone();
        then_state.path = path_and(&state.path, &condition);
        let then_value = match conditional.then {
            Some(value) => self.eval_expr(value, &mut then_state),
            None => condition_value.clone(),
        };

        let mut else_state = state.clone();
        else_state.path = path_and(&state.path, &path_not(&condition));
        let else_value = self.eval_expr(conditional.r#else, &mut else_state);

        match condition.as_bool() {
            Some(true) => {
                self.merge_expression_state(state, &then_state, true);
                then_value
            }
            Some(false) => {
                self.merge_expression_state(state, &else_state, true);
                else_value
            }
            None => {
                self.merge_expression_state(state, &then_state, false);
                self.merge_expression_state(state, &else_state, false);
                if !then_state.reachable && !else_state.reachable {
                    state.path = Bool::from_bool(false);
                    state.reachable = false;
                }
                Value::merge(Some(&condition), &then_value, &else_value)
            }
        }
    }

    fn merge_expression_state(&self, state: &mut State, branch: &State, definite: bool) {
        for name in &branch.modified {
            let env_changed = state.env.get(name) != branch.env.get(name);
            let array_values_changed =
                state.array_values.get(name) != branch.array_values.get(name);
            let tracks_array =
                state.array_values.contains_key(name) || branch.array_values.contains_key(name);
            if env_changed || array_values_changed || tracks_array {
                state.havoc(name);
            }
        }
        if definite {
            state.reachable = branch.reachable;
            if branch.reachable {
                state.path = branch.path.clone();
            } else {
                state.path = Bool::from_bool(false);
            }
        }
    }

    fn function_summary(&self, expression: &Expression<'_>) -> Option<&FunctionSummary> {
        let name = function_identifier_name(expression)?;
        if name.contains('\\') {
            return self.summaries.get(&name);
        }

        let namespace = self.namespace.clone().unwrap_or_default();
        if let Some(import) = self
            .function_imports
            .get(&namespace)
            .and_then(|imports| imports.get(&name))
        {
            return self.summaries.get(import);
        }

        if let Some(namespace) = &self.namespace {
            if let Some(summary) = self.summaries.get(&format!("{namespace}\\{name}")) {
                return Some(summary);
            }
        }

        self.summaries.get(&name)
    }

    fn method_summary(
        &self,
        class: &str,
        method: &ClassLikeMemberSelector<'_>,
    ) -> Option<&FunctionSummary> {
        let name = member_selector_name(method)?;
        self.summaries.get(&format!("{class}::{name}"))
    }

    fn static_class_name(&self, expression: &Expression<'_>) -> Option<String> {
        match expression.unparenthesized() {
            Expression::Identifier(identifier) => match identifier {
                Identifier::FullyQualified(_) => Some(
                    String::from_utf8_lossy(identifier.value())
                        .trim_start_matches('\\')
                        .to_ascii_lowercase(),
                ),
                Identifier::Local(_) | Identifier::Qualified(_) => {
                    let namespace = self.namespace.clone().unwrap_or_default();
                    match self.imports.get(&namespace) {
                        Some(imports) => Some(summary::resolve_name(
                            identifier,
                            self.namespace.as_deref(),
                            imports,
                        )),
                        None => Some(summary::resolve_name(
                            identifier,
                            self.namespace.as_deref(),
                            &HashMap::new(),
                        )),
                    }
                }
            },
            Expression::Self_(_) | Expression::Static(_) => self.class.clone(),
            Expression::Parent(_) => self
                .class
                .as_ref()
                .and_then(|class| self.parents.get(class))
                .cloned(),
            _ => None,
        }
    }

    fn eval_call(&mut self, call: &'arena Call<'arena>, state: &mut State) -> Value {
        match call {
            Call::Function(function) => {
                let arguments = self.eval_arguments(&function.argument_list, state);
                let name = function_name(function.function);
                if !is_pure_builtin(name.as_deref()) && !is_resource_builtin(name.as_deref()) {
                    self.invalidate_mutated_array_arguments(&function.argument_list, state);
                    self.invalidate_resource_arguments(&function.argument_list, state);
                }
                if is_resource_builtin(name.as_deref()) {
                    self.check_stream_lifecycle(
                        name.as_deref(),
                        &function.argument_list,
                        call.span(),
                        state,
                    );
                }
                if self.settings.check_intdiv
                    && name.as_deref() == Some("intdiv")
                    && arguments.len() >= 2
                {
                    self.check_intdiv(&arguments[0], &arguments[1], call.span(), &state.path);
                }
                if self.settings.check_assertions && name.as_deref() == Some("assert") {
                    if let Some(condition) = arguments.first().and_then(Value::truthiness) {
                        self.check_assertion(&condition, &state.path, call.span());
                        state.path = path_and(&state.path, &condition);
                    }
                }
                if arguments_are_positional(&function.argument_list) {
                    if let Some(value) = self.array_key_boundary_value(
                        name.as_deref(),
                        &function.argument_list,
                        state,
                    ) {
                        return value;
                    }
                    if let Some(value) = self.array_keys_search_value(
                        name.as_deref(),
                        &function.argument_list,
                        &arguments,
                        state,
                    ) {
                        return value;
                    }
                    if name.as_deref() == Some("array_filter") {
                        if let Some(value) = self.array_filter_value(&function.argument_list, state)
                        {
                            return value;
                        }
                    }
                    let tag = self.fresh_tag();
                    if let Some(value) = builtin_call_value(name.as_deref(), &arguments, tag) {
                        if let Some(constraint) =
                            builtin_call_constraint(name.as_deref(), &arguments, tag)
                        {
                            state.path = path_and(&state.path, &constraint);
                        }
                        return value;
                    }
                }
                let tag = self.fresh_tag();
                if let Some(summary) = self.function_summary(function.function) {
                    self.apply_call_assertions(summary, &arguments, &function.argument_list, state);
                    if let Some((value, constraint)) = summary_call_value(
                        summary,
                        &arguments,
                        arguments_are_positional(&function.argument_list),
                        tag,
                    ) {
                        if let Some(constraint) = constraint {
                            state.path = path_and(&state.path, &constraint);
                        }
                        self.apply_never_returns(summary, state);
                        return value;
                    }
                    self.apply_never_returns(summary, state);
                }
                Value::Unknown
            }
            Call::Method(method) => {
                let object = self.eval_expr(method.object, state);
                if self.settings.check_nullability
                    && !self.suppress_access_checks
                    && matches!(object, Value::Null)
                {
                    self.push_finding(
                        FindingClass::NullMethod,
                        "null-method-call",
                        "Method call on null.",
                        method.object.span(),
                        "null",
                    );
                }
                let arguments = self.eval_arguments(&method.argument_list, state);
                self.invalidate_mutated_array_arguments(&method.argument_list, state);
                self.invalidate_resource_arguments(&method.argument_list, state);
                let class = if direct_variable_name(method.object).as_deref() == Some("$this") {
                    self.class.clone()
                } else {
                    match &object {
                        Value::Object(class) => Some(class.clone()),
                        _ => None,
                    }
                };
                let tag = self.fresh_tag();
                if let Some(summary) = class
                    .as_deref()
                    .and_then(|class| self.method_summary(class, &method.method))
                {
                    self.apply_call_assertions(summary, &arguments, &method.argument_list, state);
                    self.apply_never_returns(summary, state);
                    if let Some((value, constraint)) = summary_call_value(
                        summary,
                        &arguments,
                        arguments_are_positional(&method.argument_list),
                        tag,
                    ) {
                        if let Some(constraint) = constraint {
                            state.path = path_and(&state.path, &constraint);
                        }
                        return value;
                    }
                }
                Value::Unknown
            }
            Call::NullSafeMethod(method) => {
                let object = self.eval_expr(method.object, state);
                let arguments = self.eval_arguments(&method.argument_list, state);
                self.invalidate_mutated_array_arguments(&method.argument_list, state);
                self.invalidate_resource_arguments(&method.argument_list, state);
                let class = if direct_variable_name(method.object).as_deref() == Some("$this") {
                    self.class.clone()
                } else {
                    match &object {
                        Value::Object(class) => Some(class.clone()),
                        _ => None,
                    }
                };
                let tag = self.fresh_tag();
                if let Some(summary) = class
                    .as_deref()
                    .and_then(|class| self.method_summary(class, &method.method))
                {
                    self.apply_call_assertions(summary, &arguments, &method.argument_list, state);
                    self.apply_never_returns(summary, state);
                    if let Some((value, constraint)) = summary_call_value(
                        summary,
                        &arguments,
                        arguments_are_positional(&method.argument_list),
                        tag,
                    ) {
                        if let Some(constraint) = constraint {
                            state.path = path_and(&state.path, &constraint);
                        }
                        return value;
                    }
                }
                Value::Unknown
            }
            Call::StaticMethod(method) => {
                let class = self.static_class_name(method.class);
                self.eval_expr(method.class, state);
                let arguments = self.eval_arguments(&method.argument_list, state);
                self.invalidate_mutated_array_arguments(&method.argument_list, state);
                self.invalidate_resource_arguments(&method.argument_list, state);
                let tag = self.fresh_tag();
                if let Some(summary) = class
                    .as_deref()
                    .and_then(|class| self.method_summary(class, &method.method))
                {
                    self.apply_call_assertions(summary, &arguments, &method.argument_list, state);
                    self.apply_never_returns(summary, state);
                    if let Some((value, constraint)) = summary_call_value(
                        summary,
                        &arguments,
                        arguments_are_positional(&method.argument_list),
                        tag,
                    ) {
                        if let Some(constraint) = constraint {
                            state.path = path_and(&state.path, &constraint);
                        }
                        return value;
                    }
                }
                Value::Unknown
            }
        }
    }

    fn apply_never_returns(&self, summary: &FunctionSummary, state: &mut State) {
        if summary.never_returns {
            state.path = Bool::from_bool(false);
            state.reachable = false;
        }
    }

    fn apply_call_assertions(
        &self,
        summary: &FunctionSummary,
        arguments: &[Value],
        argument_list: &'arena ArgumentList<'arena>,
        state: &mut State,
    ) {
        if summary.asserts.is_empty() || !arguments_are_positional(argument_list) {
            return;
        }

        let positional = argument_list
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument),
                Argument::Named(_) => None,
            })
            .collect::<Vec<_>>();

        for assertion in &summary.asserts {
            let Some(value) = arguments.get(assertion.index).cloned() else {
                continue;
            };
            let name = positional
                .get(assertion.index)
                .and_then(|argument| direct_variable_name(argument.value));
            let value = if !assertion.negated
                && matches!(
                    value,
                    Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. }
                ) {
                let value = parameter_value(
                    None,
                    Some(&assertion.ty),
                    "assertion",
                    name.as_deref()
                        .unwrap_or("value")
                        .strip_prefix('$')
                        .unwrap_or("value"),
                );
                if let Some(name) = &name {
                    state.env.insert(name.clone(), value.clone());
                }
                value
            } else {
                value
            };

            if let Some(constraint) = contracts::constraint(&value, &assertion.ty) {
                let constraint = if assertion.negated {
                    path_not(&constraint)
                } else {
                    constraint
                };
                state.path = path_and(&state.path, &constraint);
            } else if assertion.negated && assertion.ty == "null" && matches!(value, Value::Null) {
                state.path = Bool::from_bool(false);
                state.reachable = false;
            }
        }
    }

    fn eval_arguments(
        &mut self,
        arguments: &'arena ArgumentList<'arena>,
        state: &mut State,
    ) -> Vec<Value> {
        let mut values = Vec::new();
        for argument in arguments.arguments.iter() {
            match argument {
                Argument::Positional(argument) => {
                    values.push(self.eval_expr(argument.value, state));
                }
                Argument::Named(argument) => {
                    values.push(self.eval_expr(argument.value, state));
                }
            }
        }
        values
    }

    fn invalidate_mutated_array_arguments(&self, arguments: &ArgumentList<'_>, state: &mut State) {
        for argument in arguments.arguments.iter() {
            let expression = match argument {
                Argument::Positional(argument) => argument.value,
                Argument::Named(argument) => argument.value,
            };
            if let Some(name) = root_variable_name(expression) {
                if matches!(
                    state.env.get(&name),
                    Some(Value::Array(_) | Value::ArrayLength(_) | Value::ListLength(_))
                ) {
                    state.assign(name, Value::Unknown);
                }
            }
        }
    }

    fn invalidate_resource_arguments(&self, arguments: &ArgumentList<'_>, state: &mut State) {
        let mut ids = Vec::new();
        for argument in arguments.arguments.iter() {
            let expression = match argument {
                Argument::Positional(argument) => argument.value,
                Argument::Named(argument) => argument.value,
            };
            if let Some(name) = root_variable_name(expression) {
                if let Some(Value::Resource { id, .. }) = state.env.get(&name) {
                    ids.push(*id);
                }
            }
        }
        for id in ids {
            self.invalidate_resource_id(id, state);
        }
    }

    fn invalidate_resource_id(&self, id: u64, state: &mut State) {
        let names = state
            .env
            .iter()
            .filter(|(_, value)| {
                matches!(value, Value::Resource { id: candidate, .. } if *candidate == id)
            })
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        for name in names {
            state.assign(name, Value::Unknown);
        }
    }

    fn check_stream_lifecycle(
        &mut self,
        name: Option<&str>,
        arguments: &'arena ArgumentList<'arena>,
        span: Span,
        state: &mut State,
    ) {
        let Some(name) = name else {
            return;
        };
        if !matches!(name, "fclose" | "fread" | "fwrite") {
            return;
        }
        let expression = match arguments.arguments.first() {
            Some(Argument::Positional(argument)) => argument.value,
            _ => return,
        };
        let Some((_, id, resource_state)) = self.resource_for_expression(expression, state) else {
            return;
        };

        match name {
            "fclose" => {
                let finding = match resource_state {
                    ResourceState::Closed => {
                        Some(("stream-double-close", "The stream has already been closed."))
                    }
                    ResourceState::MaybeClosed => Some((
                        "stream-maybe-double-close",
                        "The stream may already have been closed on another path.",
                    )),
                    ResourceState::Open => None,
                };
                if let Some((id, template)) = finding {
                    let message =
                        Message::new(FindingClass::StreamLifecycle.message_id(id), template);
                    self.push_message(FindingClass::StreamLifecycle, id, message, span);
                }
                self.set_resource_state(id, ResourceState::Closed, state);
            }
            "fread" | "fwrite" => {
                let finding = match resource_state {
                    ResourceState::Closed => Some((
                        "stream-use-after-close",
                        "The stream has already been closed.",
                    )),
                    ResourceState::MaybeClosed => Some((
                        "stream-maybe-use-after-close",
                        "The stream may have been closed on another path.",
                    )),
                    ResourceState::Open => None,
                };
                if let Some((id, template)) = finding {
                    let message =
                        Message::new(FindingClass::StreamLifecycle.message_id(id), template);
                    self.push_message(FindingClass::StreamLifecycle, id, message, span);
                }
            }
            _ => {}
        }
    }

    fn resource_for_expression(
        &self,
        expression: &Expression<'_>,
        state: &State,
    ) -> Option<(String, u64, ResourceState)> {
        let name = direct_variable_name(expression)?;
        match state.env.get(&name)? {
            Value::Resource { id, state } => Some((name, *id, *state)),
            _ => None,
        }
    }

    fn set_resource_state(&self, id: u64, resource_state: ResourceState, state: &mut State) {
        let mut names = Vec::new();
        for (name, value) in state.env.iter_mut() {
            if let Value::Resource {
                id: candidate,
                state: current,
            } = value
            {
                if *candidate == id {
                    *current = resource_state;
                    names.push(name.clone());
                }
            }
        }
        for name in names {
            state.modified.insert(name);
        }
    }

    fn array_filter_value(
        &mut self,
        arguments: &'arena ArgumentList<'arena>,
        state: &State,
    ) -> Option<Value> {
        let pairs = self.array_filter_pairs(arguments, state)?;
        Some(Value::ArrayLength(Int::from_i64(pairs.len() as i64)))
    }

    fn array_filter_pairs(
        &mut self,
        arguments: &'arena ArgumentList<'arena>,
        state: &State,
    ) -> Option<Vec<(Value, Value)>> {
        if !arguments_are_positional(arguments) {
            return None;
        }
        let first = match arguments.arguments.first()? {
            Argument::Positional(argument) => argument.value,
            Argument::Named(_) => return None,
        };
        let pairs = self.array_pairs_for_expression(first, state)?;
        let callback = match arguments.arguments.get(1) {
            None => None,
            Some(Argument::Positional(argument)) => {
                if matches!(
                    argument.value.unparenthesized(),
                    Expression::Literal(Literal::Null(_))
                ) {
                    None
                } else {
                    Some(argument.value)
                }
            }
            Some(Argument::Named(_)) => return None,
        };

        let mut filtered = Vec::new();
        for (key, value) in pairs {
            let keep = match callback {
                None => value.truthiness()?.as_bool()?,
                Some(callback) => self
                    .filter_predicate_value(callback, &value, state)?
                    .as_bool()?,
            };
            if keep {
                filtered.push((key, value));
            }
        }
        Some(filtered)
    }

    fn filter_predicate_value(
        &mut self,
        callback: &'arena Expression<'arena>,
        value: &Value,
        state: &State,
    ) -> Option<Bool> {
        let (expression, body_state) = match callback.unparenthesized() {
            Expression::ArrowFunction(arrow) => {
                let mut body_state = state.clone();
                if let Some(parameter) = arrow.parameter_list.parameters.first() {
                    body_state.env.insert(
                        String::from_utf8_lossy(parameter.variable.name).to_string(),
                        value.clone(),
                    );
                }
                (arrow.expression, body_state)
            }
            Expression::Closure(closure) => {
                let expression = match closure.body.statements.as_slice() {
                    [Statement::Return(return_statement)] => return_statement.value?,
                    _ => return None,
                };
                let mut body_state = State::new();
                if let Some(use_clause) = &closure.use_clause {
                    for variable in use_clause.variables.iter() {
                        let name = String::from_utf8_lossy(variable.variable.name).to_string();
                        if let Some(value) = state.env.get(&name) {
                            body_state.env.insert(name, value.clone());
                        }
                    }
                }
                if let Some(parameter) = closure.parameter_list.parameters.first() {
                    body_state.env.insert(
                        String::from_utf8_lossy(parameter.variable.name).to_string(),
                        value.clone(),
                    );
                }
                (expression, body_state)
            }
            _ => return None,
        };
        let mut body_state = body_state;
        self.eval_expr(expression, &mut body_state).truthiness()
    }

    fn array_pairs_for_expression(
        &mut self,
        expression: &'arena Expression<'arena>,
        state: &State,
    ) -> Option<Vec<(Value, Value)>> {
        match expression.unparenthesized() {
            Expression::Array(array) => {
                let mut temp = state.clone();
                self.eval_array_pairs(array.elements.as_slice(), &mut temp)
            }
            Expression::LegacyArray(array) => {
                let mut temp = state.clone();
                self.eval_array_pairs(array.elements.as_slice(), &mut temp)
            }
            Expression::Variable(Variable::Direct(variable)) => {
                let name = String::from_utf8_lossy(variable.name).to_string();
                state.array_values.get(&name).cloned()
            }
            Expression::Call(Call::Function(function))
                if function_name(function.function).as_deref() == Some("array_filter") =>
            {
                self.array_filter_pairs(&function.argument_list, state)
            }
            _ => None,
        }
    }

    fn array_key_boundary_value(
        &self,
        name: Option<&str>,
        arguments: &ArgumentList<'_>,
        state: &State,
    ) -> Option<Value> {
        let first = match name? {
            "array_key_first" => true,
            "array_key_last" => false,
            _ => return None,
        };
        if arguments.arguments.len() != 1 {
            return None;
        }
        let expression = match arguments.arguments.first()? {
            Argument::Positional(argument) => argument.value.unparenthesized(),
            Argument::Named(_) => return None,
        };
        let variable = direct_variable_name(expression)?;
        match state.env.get(&variable)? {
            Value::Array(Some(keys)) if keys.is_empty() => return Some(Value::Null),
            Value::Array(Some(_)) => {}
            _ => return None,
        }
        let pairs = state.array_values.get(&variable)?;
        let mut seen = BTreeSet::new();
        let mut ordered = Vec::new();
        for (key, _) in pairs {
            let key = array_key(key)?;
            if seen.insert(key.clone()) {
                ordered.push(key);
            }
        }
        let key = if first {
            ordered.first()?
        } else {
            ordered.last()?
        };
        match key {
            ArrayKey::Int(value) => Some(Value::Int(Int::from_i64(*value))),
            ArrayKey::String(value) => Some(Value::String(Some(value.clone()))),
        }
    }

    fn array_keys_search_value(
        &mut self,
        name: Option<&str>,
        arguments: &'arena ArgumentList<'arena>,
        values: &[Value],
        state: &State,
    ) -> Option<Value> {
        if name != Some("array_keys") || values.len() < 2 || values.len() > 3 {
            return None;
        }
        let expression = match arguments.arguments.first()? {
            Argument::Positional(argument) => argument.value,
            Argument::Named(_) => return None,
        };
        let pairs = self.array_pairs_for_expression(expression, state)?;
        let search = values.get(1)?;
        let strict = match values.get(2) {
            None => false,
            Some(Value::Bool(strict)) => strict.as_bool()?,
            _ => return None,
        };
        let mut count = 0_i64;
        for (_, value) in pairs {
            let equal = if strict {
                values_equal_strict(&value, search)?
            } else {
                values_equal_loose(&value, search)?
            };
            if equal {
                count += 1;
            }
        }
        Some(Value::ListLength(Int::from_i64(count)))
    }

    fn isset_value(
        &self,
        expression: &Expression<'_>,
        evaluated: &Value,
        state: &State,
    ) -> Option<Bool> {
        match expression.unparenthesized() {
            Expression::ArrayAccess(access) => {
                let name = direct_variable_name(access.array);
                if let Some(Value::ListLength(length)) =
                    name.as_deref().and_then(|name| state.env.get(name))
                {
                    let Value::Int(index) = self.static_index(access.index, state)? else {
                        return None;
                    };
                    return Some(Bool::and(&[
                        &index.ge(Int::from_i64(0)),
                        &index.lt(length.clone()),
                    ]));
                }

                let index = array_key(&self.static_index(access.index, state)?)?;
                if let Expression::ArrayAccess(inner) = access.array.unparenthesized() {
                    let Some(Value::Array(Some(keys))) =
                        self.array_access_static_value(inner, state)
                    else {
                        return None;
                    };
                    return (!keys.contains(&index)).then(|| Bool::from_bool(false));
                }

                let name = name?;
                match state.env.get(&name) {
                    Some(Value::Array(Some(keys))) => {
                        if !keys.contains(&index) {
                            return Some(Bool::from_bool(false));
                        }
                        let value = state.array_values.get(&name).and_then(|pairs| {
                            pairs.iter().find_map(|(candidate, value)| {
                                (array_key(candidate).as_ref() == Some(&index)).then_some(value)
                            })
                        });
                        match value {
                            Some(Value::Null) => Some(Bool::from_bool(false)),
                            Some(Value::Unknown | Value::Unverified) | None => None,
                            Some(_) => Some(Bool::from_bool(true)),
                        }
                    }
                    Some(Value::Array(None)) => None,
                    _ => None,
                }
            }
            _ => match evaluated {
                Value::Null => Some(Bool::from_bool(false)),
                Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. } => None,
                _ => Some(Bool::from_bool(true)),
            },
        }
    }

    fn array_access_static_value(&self, access: &ArrayAccess<'_>, state: &State) -> Option<Value> {
        let name = direct_variable_name(access.array)?;
        let index = array_key(&self.static_index(access.index, state)?)?;
        state.array_values.get(&name).and_then(|pairs| {
            pairs.iter().find_map(|(candidate, value)| {
                (array_key(candidate).as_ref() == Some(&index)).then(|| value.clone())
            })
        })
    }

    fn static_index(&self, expression: &Expression<'_>, state: &State) -> Option<Value> {
        match expression.unparenthesized() {
            Expression::Literal(literal) => Some(self.eval_literal(literal)),
            Expression::Variable(Variable::Direct(variable)) => state
                .env
                .get(&String::from_utf8_lossy(variable.name).to_string())
                .cloned(),
            _ => None,
        }
    }

    fn eval_construct(&mut self, construct: &'arena Construct<'arena>, state: &mut State) -> Value {
        match construct {
            Construct::Isset(isset) => {
                let previous = self.suppress_access_checks;
                self.suppress_access_checks = true;
                let mut result = Bool::from_bool(true);
                let mut unknown = false;
                for value in isset.values.iter() {
                    let evaluated = self.eval_expr(value, state);
                    match self.isset_value(value, &evaluated, state) {
                        Some(exists) => {
                            result = path_and(&result, &exists);
                        }
                        None => unknown = true,
                    }
                }
                self.suppress_access_checks = previous;
                if result.as_bool() == Some(false) {
                    Value::Bool(result)
                } else if unknown {
                    Value::Unknown
                } else {
                    Value::Bool(result)
                }
            }
            Construct::Empty(empty) => {
                let previous = self.suppress_access_checks;
                self.suppress_access_checks = true;
                self.eval_expr(empty.value, state);
                self.suppress_access_checks = previous;
                Value::Unknown
            }
            Construct::Eval(eval) => {
                self.eval_expr(eval.value, state);
                Value::Unknown
            }
            Construct::Include(include) => {
                self.eval_expr(include.value, state);
                Value::Unknown
            }
            Construct::IncludeOnce(include) => {
                self.eval_expr(include.value, state);
                Value::Unknown
            }
            Construct::Require(require) => {
                self.eval_expr(require.value, state);
                Value::Unknown
            }
            Construct::RequireOnce(require) => {
                self.eval_expr(require.value, state);
                Value::Unknown
            }
            Construct::Print(print) => {
                self.eval_expr(print.value, state);
                Value::Unknown
            }
            Construct::Exit(exit) => {
                if let Some(arguments) = &exit.arguments {
                    for argument in arguments.arguments.iter() {
                        match argument {
                            Argument::Positional(argument) => {
                                self.eval_expr(argument.value, state);
                            }
                            Argument::Named(argument) => {
                                self.eval_expr(argument.value, state);
                            }
                        }
                    }
                }
                state.path = Bool::from_bool(false);
                state.reachable = false;
                Value::Unknown
            }
            Construct::Die(die) => {
                if let Some(arguments) = &die.arguments {
                    for argument in arguments.arguments.iter() {
                        match argument {
                            Argument::Positional(argument) => {
                                self.eval_expr(argument.value, state);
                            }
                            Argument::Named(argument) => {
                                self.eval_expr(argument.value, state);
                            }
                        }
                    }
                }
                state.path = Bool::from_bool(false);
                state.reachable = false;
                Value::Unknown
            }
        }
    }

    fn eval_string(&mut self, string: &'arena CompositeString<'arena>, state: &mut State) -> Value {
        match string {
            CompositeString::Interpolated(interpolated) => {
                let mut result = String::new();
                for part in interpolated.parts.iter() {
                    match part {
                        StringPart::Literal(literal) => {
                            if let Some(value) = literal.value {
                                result.push_str(&String::from_utf8_lossy(value));
                            }
                        }
                        StringPart::Expression(expression) => {
                            self.eval_expr(expression, state);
                            return Value::String(None);
                        }
                        StringPart::BracedExpression(expression) => {
                            self.eval_expr(expression.expression, state);
                            return Value::String(None);
                        }
                    }
                }
                Value::String(Some(result))
            }
            CompositeString::Document(document) => {
                for part in document.parts.iter() {
                    match part {
                        StringPart::Literal(_) => {}
                        StringPart::Expression(expression) => {
                            self.eval_expr(expression, state);
                            return Value::String(None);
                        }
                        StringPart::BracedExpression(expression) => {
                            self.eval_expr(expression.expression, state);
                            return Value::String(None);
                        }
                    }
                }
                Value::String(None)
            }
            CompositeString::ShellExecute(_) => Value::Unknown,
        }
    }

    fn eval_array_elements(
        &mut self,
        elements: &'arena [ArrayElement<'arena>],
        state: &mut State,
    ) -> Value {
        let mut keys = BTreeSet::new();
        let mut next_index: Option<i64> = None;
        let mut known = true;

        for element in elements {
            match element {
                ArrayElement::KeyValue(element) => {
                    let key_value = self.eval_expr(element.key, state);
                    self.eval_expr(element.value, state);
                    match array_key(&key_value) {
                        Some(key) => {
                            if let ArrayKey::Int(index) = &key {
                                next_index = Some(match next_index {
                                    Some(current) => current.max(index.saturating_add(1)),
                                    None => index.saturating_add(1),
                                });
                            }
                            keys.insert(key);
                        }
                        None => known = false,
                    }
                }
                ArrayElement::Value(element) => {
                    self.eval_expr(element.value, state);
                    let index = next_index.unwrap_or(0);
                    keys.insert(ArrayKey::Int(index));
                    next_index = Some(index.saturating_add(1));
                }
                ArrayElement::Variadic(element) => {
                    self.eval_expr(element.value, state);
                    known = false;
                }
                ArrayElement::Missing(_) => {
                    next_index = Some(next_index.unwrap_or(0).saturating_add(1));
                }
            }
        }

        if known {
            Value::Array(Some(keys))
        } else {
            Value::Array(None)
        }
    }

    fn eval_array_pairs(
        &mut self,
        elements: &'arena [ArrayElement<'arena>],
        state: &mut State,
    ) -> Option<Vec<(Value, Value)>> {
        let mut pairs = Vec::new();
        let mut next_index: Option<i64> = None;

        for element in elements {
            match element {
                ArrayElement::KeyValue(element) => {
                    let key = self.eval_expr(element.key, state);
                    let value = self.eval_expr(element.value, state);
                    let normalized = array_key(&key);
                    if let Some(ArrayKey::Int(index)) = &normalized {
                        next_index = Some(match next_index {
                            Some(current) => current.max(index.saturating_add(1)),
                            None => index.saturating_add(1),
                        });
                    }
                    match normalized {
                        Some(normalized) => {
                            if let Some(pair) = pairs.iter_mut().find(|(candidate, _)| {
                                array_key(candidate).as_ref() == Some(&normalized)
                            }) {
                                pair.1 = value;
                            } else {
                                pairs.push((key, value));
                            }
                        }
                        None => pairs.push((key, value)),
                    }
                }
                ArrayElement::Value(element) => {
                    let value = self.eval_expr(element.value, state);
                    let index = next_index.unwrap_or(0);
                    pairs.push((Value::Int(Int::from_i64(index)), value));
                    next_index = Some(index.saturating_add(1));
                }
                ArrayElement::Missing(_) => {
                    next_index = Some(next_index.unwrap_or(0).saturating_add(1));
                }
                ArrayElement::Variadic(_) => return None,
            }
        }

        Some(pairs)
    }

    fn array_access_value(
        &self,
        expression: &Expression<'_>,
        array: &Value,
        index: &Value,
        state: &State,
    ) -> Value {
        let Some(name) = direct_variable_name(expression) else {
            return Value::Unknown;
        };
        let Some(pairs) = state.array_values.get(&name) else {
            return Value::Unknown;
        };
        let Some(key) = array_key(index) else {
            return Value::Unknown;
        };
        for (candidate, value) in pairs {
            if array_key(candidate).as_ref() == Some(&key) {
                return value.clone();
            }
        }
        if self.suppress_access_checks && matches!(array, Value::Array(Some(_))) {
            Value::Null
        } else {
            Value::Unknown
        }
    }

    fn check_array_access(&mut self, array: &Value, index: &Value, span: Span, path: &Bool) {
        if self.suppress_access_checks {
            return;
        }
        if !self.settings.check_array_index && !self.settings.check_string_offset {
            return;
        }

        match array {
            Value::Array(Some(keys)) => {
                if !self.settings.check_array_index {
                    return;
                }

                match array_key(index) {
                    Some(key) => {
                        if !keys.contains(&key) && path_may_be_true(path) {
                            self.push_finding(
                                FindingClass::ArrayKey,
                                "array-key",
                                "Array key {value} is not present in the array.",
                                span,
                                &render_array_key(&key),
                            );
                        }
                    }
                    None => {
                        if let Value::Int(index) = index {
                            let mut present = Vec::new();
                            for key in keys {
                                if let ArrayKey::Int(value) = key {
                                    present.push(index.eq(*value));
                                }
                            }
                            let present = if present.is_empty() {
                                Bool::from_bool(false)
                            } else {
                                Bool::or(&present.iter().collect::<Vec<_>>())
                            };
                            let index_value = Value::Int(index.clone());
                            self.check_condition(
                                FindingClass::ArrayKey,
                                &present.not(),
                                path,
                                span,
                                "array-key",
                                "Array key {value} is not present in the array.",
                                Some(&index_value),
                            );
                        }
                    }
                }
            }
            Value::String(Some(string)) => {
                if !self.settings.check_string_offset {
                    return;
                }
                let length = Int::from_i64(string.len() as i64);
                self.check_string_offset(&length, index, span, path);
            }
            Value::StringLength(length) => {
                if !self.settings.check_string_offset {
                    return;
                }
                self.check_string_offset(length, index, span, path);
            }
            Value::ListLength(length) => self.check_list_access(length, index, span, path),
            _ => {}
        }
    }

    fn check_string_offset(&mut self, length: &Int, index: &Value, span: Span, path: &Bool) {
        match index {
            Value::Int(index) => {
                let out_of_bounds =
                    Bool::or(&[&index.lt(-length.clone()), &index.ge(length.clone())]);
                let index_value = Value::Int(index.clone());
                self.check_condition(
                    FindingClass::StringOffset,
                    &out_of_bounds,
                    path,
                    span,
                    "string-offset",
                    "String offset {value} is out of bounds.",
                    Some(&index_value),
                );
            }
            Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. } => {
                if self.settings.unknown_policy != "report" {
                    return;
                }
                let solver = Solver::new();
                let mut params = Params::new();
                params.set_u32(
                    "timeout",
                    self.settings.timeout_ms.min(u32::MAX as u64) as u32,
                );
                solver.set_params(&params);
                solver.assert(path);
                if !matches!(solver.check(), SatResult::Unsat) {
                    self.push_unverified(
                        FindingClass::StringOffset,
                        "Could not verify this string offset: the index cannot be modeled. Treat it as unverified, not safe; see unknown_policy.",
                        span,
                    );
                }
            }
            _ => {}
        }
    }

    fn check_list_access(&mut self, length: &Int, index: &Value, span: Span, path: &Bool) {
        if !self.settings.check_array_index {
            return;
        }

        match index {
            Value::Int(index) => {
                let out_of_bounds =
                    Bool::or(&[&index.lt(Int::from_i64(0)), &index.ge(length.clone())]);
                let solver = Solver::new();
                let mut params = Params::new();
                params.set_u32(
                    "timeout",
                    self.settings.timeout_ms.min(u32::MAX as u64) as u32,
                );
                solver.set_params(&params);
                solver.assert(path);
                solver.assert(&out_of_bounds);

                match solver.check() {
                    SatResult::Sat => {
                        let safety_solver = Solver::new();
                        safety_solver.set_params(&params);
                        safety_solver.assert(path);
                        safety_solver.assert(out_of_bounds.not());
                        let safety = safety_solver.check();
                        let symbolic_index = index.as_i64().is_none();
                        match safety {
                            SatResult::Unsat => {
                                let index_value = model_value(&solver, &Value::Int(index.clone()))
                                    .unwrap_or_else(|| "?".to_string());
                                let length_value =
                                    model_value(&solver, &Value::Int(length.clone()))
                                        .unwrap_or_else(|| "?".to_string());
                                let message = Message::new(
                                    FindingClass::ArrayIndex.message_id("array-index"),
                                    "A reachable path uses list index {value}, but the list has length {length}.",
                                )
                                .arg("value", index_value)
                                .arg("length", length_value);
                                self.push_message(
                                    FindingClass::ArrayIndex,
                                    "array-index",
                                    message,
                                    span,
                                );
                            }
                            SatResult::Sat if symbolic_index => {
                                let index_value = model_value(&solver, &Value::Int(index.clone()))
                                    .unwrap_or_else(|| "?".to_string());
                                let length_value =
                                    model_value(&solver, &Value::Int(length.clone()))
                                        .unwrap_or_else(|| "?".to_string());
                                let message = Message::new(
                                    FindingClass::ArrayIndex.message_id("array-index"),
                                    "A reachable path uses list index {value}, but the list has length {length}.",
                                )
                                .arg("value", index_value)
                                .arg("length", length_value);
                                self.push_message(
                                    FindingClass::ArrayIndex,
                                    "array-index",
                                    message,
                                    span,
                                );
                            }
                            SatResult::Unknown => {
                                if self.settings.unknown_policy == "report" {
                                    self.push_unverified(
                                        FindingClass::ArrayIndex,
                                        "Could not verify this list access: the solver returned unknown. Treat it as unverified, not safe; see unknown_policy.",
                                        span,
                                    );
                                }
                            }
                            SatResult::Sat => {}
                        }
                    }
                    SatResult::Unknown => {
                        if self.settings.unknown_policy == "report" {
                            self.push_unverified(
                                FindingClass::ArrayIndex,
                                "Could not verify this list access: the solver returned unknown. Treat it as unverified, not safe; see unknown_policy.",
                                span,
                            );
                        }
                    }
                    SatResult::Unsat => {}
                }
            }
            Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. } => {
                if self.settings.unknown_policy != "report" {
                    return;
                }
                let solver = Solver::new();
                let mut params = Params::new();
                params.set_u32(
                    "timeout",
                    self.settings.timeout_ms.min(u32::MAX as u64) as u32,
                );
                solver.set_params(&params);
                solver.assert(path);
                if !matches!(solver.check(), SatResult::Unsat) {
                    self.push_unverified(
                        FindingClass::ArrayIndex,
                        "Could not verify this list access: the index cannot be modeled. Treat it as unverified, not safe; see unknown_policy.",
                        span,
                    );
                }
            }
            _ => {}
        }
    }

    fn fresh_unverified_numeric(&mut self) -> Value {
        self.next_unverified_numeric += 1;
        Value::UnverifiedNumeric {
            value: Int::new_const(format!(
                "unverified::numeric::{}",
                self.next_unverified_numeric
            )),
            loop_accumulator: false,
        }
    }

    fn fresh_tag(&mut self) -> u64 {
        self.next_fresh += 1;
        self.next_fresh
    }

    fn fresh_resource_id(&mut self) -> u64 {
        self.next_resource += 1;
        self.next_resource
    }

    fn numeric_unknown_arithmetic(&mut self, left: &Value, right: &Value) -> Option<Value> {
        let known_numeric = |value: &Value| matches!(value, Value::Int(_) | Value::Float(_));
        if matches!(
            left,
            Value::UnverifiedNumeric { .. } | Value::Int(_) | Value::Float(_)
        ) && matches!(right, Value::Unknown | Value::UnverifiedNumeric { .. })
            || matches!(
                right,
                Value::UnverifiedNumeric { .. } | Value::Int(_) | Value::Float(_)
            ) && matches!(left, Value::Unknown | Value::UnverifiedNumeric { .. })
            || matches!(left, Value::UnverifiedNumeric { .. }) && known_numeric(right)
            || matches!(right, Value::UnverifiedNumeric { .. }) && known_numeric(left)
        {
            return Some(self.fresh_unverified_numeric());
        }
        None
    }

    fn add_values(&mut self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => Value::Int(left.add(right)),
            (Value::Float(left), Value::Float(right)) => Value::Float(
                left.add_with_rounding_mode(right, &RoundingMode::round_nearest_ties_to_even()),
            ),
            _ => {
                if let Some(value) = self.numeric_unknown_arithmetic(left, right) {
                    return value;
                }
                match (left.to_float(), right.to_float()) {
                    (Some(left), Some(right)) => Value::Float(left.add_with_rounding_mode(
                        &right,
                        &RoundingMode::round_nearest_ties_to_even(),
                    )),
                    _ => Value::Unknown,
                }
            }
        }
    }

    fn sub_values(&mut self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => Value::Int(left.sub(right)),
            (Value::Float(left), Value::Float(right)) => Value::Float(
                left.sub_with_rounding_mode(right, &RoundingMode::round_nearest_ties_to_even()),
            ),
            _ => {
                if let Some(value) = self.numeric_unknown_arithmetic(left, right) {
                    return value;
                }
                match (left.to_float(), right.to_float()) {
                    (Some(left), Some(right)) => Value::Float(left.sub_with_rounding_mode(
                        &right,
                        &RoundingMode::round_nearest_ties_to_even(),
                    )),
                    _ => Value::Unknown,
                }
            }
        }
    }

    fn mul_values(&mut self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => Value::Int(left.mul(right)),
            (Value::Float(left), Value::Float(right)) => Value::Float(
                left.mul_with_rounding_mode(right, &RoundingMode::round_nearest_ties_to_even()),
            ),
            _ => {
                if let Some(value) = self.numeric_unknown_arithmetic(left, right) {
                    return value;
                }
                match (left.to_float(), right.to_float()) {
                    (Some(left), Some(right)) => Value::Float(left.mul_with_rounding_mode(
                        &right,
                        &RoundingMode::round_nearest_ties_to_even(),
                    )),
                    _ => Value::Unknown,
                }
            }
        }
    }

    fn divide_values(&self, left: &Value, right: &Value) -> Value {
        match (left.to_float(), right.to_float()) {
            (Some(left), Some(right)) => Value::Float(
                left.div_with_rounding_mode(&right, &RoundingMode::round_nearest_ties_to_even()),
            ),
            _ => Value::Unknown,
        }
    }

    fn mod_values(&self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => Value::Int(left.rem(right)),
            _ => Value::Unknown,
        }
    }

    fn concat_values(&self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::String(Some(left)), Value::String(Some(right))) => {
                Value::String(Some(format!("{left}{right}")))
            }
            (Value::StringLength(left), Value::StringLength(right)) => {
                Value::StringLength(left.add(right))
            }
            (Value::StringLength(left), Value::String(Some(right))) => {
                Value::StringLength(left.add(Int::from_i64(right.len() as i64)))
            }
            (Value::String(Some(left)), Value::StringLength(right)) => {
                Value::StringLength(Int::from_i64(left.len() as i64).add(right))
            }
            _ => Value::Unknown,
        }
    }

    fn compare_equal(&self, left: &Value, right: &Value, negate: bool) -> Value {
        let comparison = match (left, right) {
            (Value::Int(left), Value::Int(right)) => Some(left.eq(right)),
            (Value::Float(left), Value::Float(right)) => Some(left.eq_fpa(right)),
            (Value::Int(left), Value::Float(right)) => Some(match int_to_float(left) {
                Some(left) => left.eq_fpa(right),
                None => left.to_real().eq(right.to_real()),
            }),
            (Value::Float(left), Value::Int(right)) => Some(match int_to_float(right) {
                Some(right) => left.eq_fpa(&right),
                None => left.to_real().eq(right.to_real()),
            }),
            (Value::UnverifiedNumeric { value: left, .. }, Value::Int(right)) => {
                Some(left.eq(right))
            }
            (
                Value::UnverifiedNumeric { value: left, .. },
                Value::UnverifiedNumeric { value: right, .. },
            ) => Some(left.eq(right)),
            (Value::Bool(left), Value::Bool(right)) => Some(left.eq(right)),
            (Value::ArrayLength(length) | Value::ListLength(length), Value::Array(Some(keys)))
            | (Value::Array(Some(keys)), Value::ArrayLength(length) | Value::ListLength(length))
                if keys.is_empty() =>
            {
                Some(length.eq(Int::from_i64(0)))
            }
            (Value::Null, Value::Null) => Some(Bool::from_bool(true)),
            _ => None,
        };
        comparison
            .map(|comparison| Value::Bool(if negate { comparison.not() } else { comparison }))
            .unwrap_or(Value::Unknown)
    }

    fn compare_identical(&self, left: &Value, right: &Value, negate: bool) -> Value {
        if matches!(
            left,
            Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. }
        ) || matches!(
            right,
            Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. }
        ) {
            return Value::Unknown;
        }

        let comparison = match (left, right) {
            (Value::Int(left), Value::Int(right)) => Some(left.eq(right)),
            (Value::Float(left), Value::Float(right)) => Some(left.eq_fpa(right)),
            (Value::Bool(left), Value::Bool(right)) => Some(left.eq(right)),
            (Value::String(Some(left)), Value::String(Some(right))) => {
                Some(Bool::from_bool(left == right))
            }
            (Value::ArrayLength(length) | Value::ListLength(length), Value::Array(Some(keys)))
            | (Value::Array(Some(keys)), Value::ArrayLength(length) | Value::ListLength(length))
                if keys.is_empty() =>
            {
                Some(length.eq(Int::from_i64(0)))
            }
            (Value::Null, Value::Null) => Some(Bool::from_bool(true)),
            (Value::Int(_), Value::Float(_))
            | (Value::Float(_), Value::Int(_))
            | (Value::Int(_), Value::String(_))
            | (Value::String(_), Value::Int(_))
            | (Value::Float(_), Value::String(_))
            | (Value::String(_), Value::Float(_))
            | (Value::Int(_), Value::StringLength(_))
            | (Value::StringLength(_), Value::Int(_))
            | (Value::Float(_), Value::StringLength(_))
            | (Value::StringLength(_), Value::Float(_))
            | (Value::Null, _)
            | (_, Value::Null) => Some(Bool::from_bool(false)),
            _ => None,
        };
        comparison
            .map(|comparison| Value::Bool(if negate { comparison.not() } else { comparison }))
            .unwrap_or(Value::Unknown)
    }

    fn compare_less(&self, left: &Value, right: &Value, or_equal: bool) -> Value {
        let comparison = match (left, right) {
            (Value::Int(left), Value::Int(right)) => Some(if or_equal {
                left.le(right)
            } else {
                left.lt(right)
            }),
            (Value::Float(left), Value::Float(right)) => Some(if or_equal {
                left.le(right)
            } else {
                left.lt(right)
            }),
            (Value::Int(left), Value::Float(right)) => Some(match int_to_float(left) {
                Some(left) => {
                    if or_equal {
                        left.le(right)
                    } else {
                        left.lt(right)
                    }
                }
                None => {
                    if or_equal {
                        left.to_real().le(right.to_real())
                    } else {
                        left.to_real().lt(right.to_real())
                    }
                }
            }),
            (Value::Float(left), Value::Int(right)) => Some(match int_to_float(right) {
                Some(right) => {
                    if or_equal {
                        left.le(&right)
                    } else {
                        left.lt(&right)
                    }
                }
                None => {
                    if or_equal {
                        left.to_real().le(right.to_real())
                    } else {
                        left.to_real().lt(right.to_real())
                    }
                }
            }),
            (Value::UnverifiedNumeric { value: left, .. }, Value::Int(right)) => {
                Some(if or_equal {
                    left.le(right)
                } else {
                    left.lt(right)
                })
            }
            (Value::Int(left), Value::UnverifiedNumeric { value: right, .. }) => {
                Some(if or_equal {
                    left.le(right)
                } else {
                    left.lt(right)
                })
            }
            (
                Value::UnverifiedNumeric { value: left, .. },
                Value::UnverifiedNumeric { value: right, .. },
            ) => Some(if or_equal {
                left.le(right)
            } else {
                left.lt(right)
            }),
            _ => None,
        };
        comparison.map(Value::Bool).unwrap_or(Value::Unknown)
    }

    fn compare_greater(&self, left: &Value, right: &Value, or_equal: bool) -> Value {
        let comparison = match (left, right) {
            (Value::Int(left), Value::Int(right)) => Some(if or_equal {
                left.ge(right)
            } else {
                left.gt(right)
            }),
            (Value::Float(left), Value::Float(right)) => Some(if or_equal {
                left.ge(right)
            } else {
                left.gt(right)
            }),
            (Value::Int(left), Value::Float(right)) => Some(match int_to_float(left) {
                Some(left) => {
                    if or_equal {
                        left.ge(right)
                    } else {
                        left.gt(right)
                    }
                }
                None => {
                    if or_equal {
                        left.to_real().ge(right.to_real())
                    } else {
                        left.to_real().gt(right.to_real())
                    }
                }
            }),
            (Value::Float(left), Value::Int(right)) => Some(match int_to_float(right) {
                Some(right) => {
                    if or_equal {
                        left.ge(&right)
                    } else {
                        left.gt(&right)
                    }
                }
                None => {
                    if or_equal {
                        left.to_real().ge(right.to_real())
                    } else {
                        left.to_real().gt(right.to_real())
                    }
                }
            }),
            (Value::UnverifiedNumeric { value: left, .. }, Value::Int(right)) => {
                Some(if or_equal {
                    left.ge(right)
                } else {
                    left.gt(right)
                })
            }
            (Value::Int(left), Value::UnverifiedNumeric { value: right, .. }) => {
                Some(if or_equal {
                    left.ge(right)
                } else {
                    left.gt(right)
                })
            }
            (
                Value::UnverifiedNumeric { value: left, .. },
                Value::UnverifiedNumeric { value: right, .. },
            ) => Some(if or_equal {
                left.ge(right)
            } else {
                left.gt(right)
            }),
            _ => None,
        };
        comparison.map(Value::Bool).unwrap_or(Value::Unknown)
    }

    fn increment_value(&self, value: &Value, increment: bool) -> Value {
        match value {
            Value::Int(value) => {
                if increment {
                    Value::Int(value.add(Int::from_i64(1)))
                } else {
                    Value::Int(value.sub(Int::from_i64(1)))
                }
            }
            Value::Float(value) => {
                if increment {
                    Value::Float(value.add_with_rounding_mode(
                        Float::from_f64(1.0),
                        &RoundingMode::round_nearest_ties_to_even(),
                    ))
                } else {
                    Value::Float(value.sub_with_rounding_mode(
                        Float::from_f64(1.0),
                        &RoundingMode::round_nearest_ties_to_even(),
                    ))
                }
            }
            Value::UnverifiedNumeric {
                value,
                loop_accumulator,
            } => {
                if increment {
                    Value::UnverifiedNumeric {
                        value: value.add(Int::from_i64(1)),
                        loop_accumulator: *loop_accumulator,
                    }
                } else {
                    Value::UnverifiedNumeric {
                        value: value.sub(Int::from_i64(1)),
                        loop_accumulator: *loop_accumulator,
                    }
                }
            }
            _ => Value::Unknown,
        }
    }

    fn check_return_contract(&mut self, value: Option<&Value>, span: Span) {
        if self.in_generator {
            return;
        }
        if let Some(hint) = self.return_hint {
            self.check_native_return_contract(value, hint, span);
        }
        if let Some(doc_type) = self.doc_return.clone() {
            self.check_doc_return_contract(value, &doc_type, span);
        }
    }

    fn check_native_return_contract(&mut self, value: Option<&Value>, hint: &Hint<'_>, span: Span) {
        let Some(value) = value else {
            if !matches!(hint, Hint::Void(_) | Hint::Never(_) | Hint::Mixed(_))
                && !hint.contains_null()
            {
                self.push_message(
                    FindingClass::ReturnContract,
                    "return-contract",
                    Message::new(
                        FindingClass::ReturnContract.message_id("return-contract"),
                        "The function can return without a value but declares return type {type}.",
                    )
                    .arg("type", hint_name(hint)),
                    span,
                );
            }
            return;
        };

        if matches!(
            value,
            Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. } | Value::Object(_)
        ) {
            return;
        }

        if hint_accepts_value(value, hint, self.strict_types) == Some(false) {
            self.push_message(
                FindingClass::ReturnContract,
                "return-contract",
                Message::new(
                    FindingClass::ReturnContract.message_id("return-contract"),
                    "Returned {value} does not satisfy the declared return type {type}.",
                )
                .arg("value", value_type_name(value))
                .arg("type", hint_name(hint)),
                span,
            );
        }
    }

    fn check_doc_return_contract(&mut self, value: Option<&Value>, doc_type: &str, span: Span) {
        let Some(value) = value else {
            if doc_type != "void"
                && doc_type != "never"
                && !contracts::accepts_null(doc_type, &self.doc_templates)
            {
                self.push_message(
                    FindingClass::ReturnContract,
                    "return-contract",
                    Message::new(
                        FindingClass::ReturnContract.message_id("return-contract"),
                        "The function can return without a value but documents return type {type}.",
                    )
                    .arg("type", doc_type),
                    span,
                );
            }
            return;
        };

        if matches!(
            value,
            Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. } | Value::Object(_)
        ) {
            return;
        }

        if contracts::accepts_value(value, doc_type, true) == Some(false) {
            self.push_message(
                FindingClass::ReturnContract,
                "return-contract",
                Message::new(
                    FindingClass::ReturnContract.message_id("return-contract"),
                    "Returned {value} does not satisfy the documented return type {type}.",
                )
                .arg("value", value_type_name(value))
                .arg("type", doc_type),
                span,
            );
        }
    }

    fn check_intdiv(&mut self, left: &Value, right: &Value, span: Span, path: &Bool) {
        if !matches!(
            right,
            Value::Int(_) | Value::Unverified | Value::UnverifiedNumeric { .. }
        ) {
            return;
        }
        self.check_nonzero(
            FindingClass::Intdiv,
            right,
            path,
            span,
            "intdiv-by-zero",
            "intdiv() by zero: the divisor can be {value}.",
        );

        if let (Value::Int(left), Value::Int(right)) = (left, right) {
            let overflow = Bool::and(&[
                &left.eq(Int::from_i64(i64::MIN)),
                &right.eq(Int::from_i64(-1)),
            ]);
            self.check_condition(
                FindingClass::IntdivOverflow,
                &overflow,
                path,
                span,
                "intdiv-overflow",
                "intdiv() overflows for PHP_INT_MIN / -1.",
                None,
            );
        }
    }

    fn check_nonzero(
        &mut self,
        class: FindingClass,
        value: &Value,
        path: &Bool,
        span: Span,
        id: &'static str,
        template: &'static str,
    ) {
        if self.settings.unknown_policy == "report" {
            if let Value::UnverifiedNumeric {
                value,
                loop_accumulator,
            } = value
            {
                if !loop_accumulator {
                    return;
                }
                let solver = Solver::new();
                let mut params = Params::new();
                params.set_u32(
                    "timeout",
                    self.settings.timeout_ms.min(u32::MAX as u64) as u32,
                );
                solver.set_params(&params);
                solver.assert(path);
                solver.assert(value.eq(0));
                if !matches!(solver.check(), SatResult::Unsat) {
                    self.push_unverified(
                        class,
                        "Could not verify this property: the divisor may be zero but its numeric value cannot be modeled. Treat it as unverified, not safe; see unknown_policy.",
                        span,
                    );
                }
                return;
            }
            if matches!(value, Value::Unverified) {
                self.push_unverified(
                    class,
                    "Could not verify this property: the divisor cannot be modeled. Treat it as unverified, not safe; see unknown_policy.",
                    span,
                );
                return;
            }
        } else if matches!(value, Value::Unverified | Value::UnverifiedNumeric { .. }) {
            return;
        }
        let Some(zero) = value.zero_condition() else {
            return;
        };

        self.check_condition(class, &zero, path, span, id, template, Some(value));
    }

    fn check_assertion(&mut self, condition: &Bool, path: &Bool, span: Span) {
        let path_solver = Solver::new();
        let mut params = Params::new();
        params.set_u32(
            "timeout",
            self.settings.timeout_ms.min(u32::MAX as u64) as u32,
        );
        path_solver.set_params(&params);
        path_solver.assert(path);
        if !matches!(path_solver.check(), SatResult::Sat) {
            return;
        }

        let solver = Solver::new();
        solver.set_params(&params);
        solver.assert(path);
        solver.assert(condition);

        if matches!(solver.check(), SatResult::Unsat) {
            self.push_finding(
                FindingClass::Assertion,
                "assertion",
                "This assertion can never hold.",
                span,
                "false",
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_condition(
        &mut self,
        class: FindingClass,
        condition: &Bool,
        path: &Bool,
        span: Span,
        id: &'static str,
        template: &'static str,
        value: Option<&Value>,
    ) {
        let solver = Solver::new();
        let mut params = Params::new();
        params.set_u32(
            "timeout",
            self.settings.timeout_ms.min(u32::MAX as u64) as u32,
        );
        solver.set_params(&params);
        solver.assert(path);
        solver.assert(condition);

        match solver.check() {
            SatResult::Sat => {
                let rendered = value
                    .and_then(|value| model_value(&solver, value))
                    .unwrap_or_else(|| "?".to_string());
                self.push_finding(class, id, template, span, &rendered);
            }
            SatResult::Unknown => {
                if self.settings.unknown_policy == "report" {
                    self.push_unverified(
                        class,
                        "Could not verify this property: the solver returned unknown. Treat it as unverified, not safe; see unknown_policy.",
                        span,
                    );
                }
            }
            SatResult::Unsat => {}
        }
    }

    fn push_finding(
        &mut self,
        class: FindingClass,
        id: &'static str,
        template: &'static str,
        span: Span,
        value: &str,
    ) {
        let message = Message::new(class.message_id(id), template).arg("value", value);
        self.push_message(class, id, message, span);
    }

    fn push_unverified(&mut self, class: FindingClass, template: &'static str, span: Span) {
        self.push_finding(class, "unverified", template, span, "?");
    }

    fn push_message(
        &mut self,
        class: FindingClass,
        id: &'static str,
        message: Message,
        span: Span,
    ) {
        if !self.classes.is_empty() && !self.classes.contains(&class) {
            return;
        }

        if self.settings.max_findings_per_function > 0 {
            let class_findings = self.findings[self.finding_limit_start..]
                .iter()
                .filter(|finding| finding.class == class)
                .count();
            if class_findings >= self.settings.max_findings_per_function {
                return;
            }
        }

        let key = (span.start.offset, span.end.offset, class, id);
        if !self.seen.insert(key) {
            return;
        }

        if id != "unverified" {
            let unverified = (span.start.offset, span.end.offset, class, "unverified");
            if self.seen.remove(&unverified) {
                let unverified_id = class.message_id("unverified");
                self.findings.retain(|finding| {
                    !(finding.class == class
                        && finding.span.start.offset == span.start.offset
                        && finding.span.end.offset == span.end.offset
                        && finding.message.id == unverified_id)
                });
            }
        }

        self.findings.push(Finding {
            class,
            message,
            span,
        });
    }
}

fn model_value(solver: &Solver, value: &Value) -> Option<String> {
    let model = solver.get_model()?;
    match value {
        Value::Int(value) => model
            .eval(value, true)
            .and_then(|value| value.as_i64())
            .map(|value| value.to_string()),
        Value::Float(value) => model
            .eval(value, true)
            .map(|value| value.as_f64().to_string()),
        Value::Bool(value) => model
            .eval(value, true)
            .and_then(|value| value.as_bool())
            .map(|value| value.to_string()),
        _ => None,
    }
}

fn member_selector_name(selector: &ClassLikeMemberSelector<'_>) -> Option<String> {
    match selector {
        ClassLikeMemberSelector::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.value).to_ascii_lowercase())
        }
        _ => None,
    }
}

fn function_identifier_name(expression: &Expression<'_>) -> Option<String> {
    match expression.unparenthesized() {
        Expression::Identifier(identifier) => Some(
            String::from_utf8_lossy(identifier.value())
                .trim_start_matches('\\')
                .to_ascii_lowercase(),
        ),
        _ => None,
    }
}

fn is_list_keys(keys: &BTreeSet<ArrayKey>) -> bool {
    keys.iter()
        .enumerate()
        .all(|(index, key)| matches!(key, ArrayKey::Int(value) if *value == index as i64))
}

fn array_length(value: &Value) -> Option<Int> {
    match value {
        Value::Array(Some(keys)) => Some(Int::from_u64(keys.len() as u64)),
        Value::ArrayLength(length) | Value::ListLength(length) => Some(length.clone()),
        _ => None,
    }
}

fn int_max(left: Int, right: Int) -> Int {
    left.ge(right.clone()).ite(&left, &right)
}

fn int_min(left: Int, right: Int) -> Int {
    left.le(right.clone()).ite(&left, &right)
}

fn int_to_float(value: &Int) -> Option<Float> {
    value.as_i64().map(|value| Float::from_f64(value as f64))
}

fn slice_length(total: Int, offset: &Int, length: Option<&Int>) -> Int {
    let zero = Int::from_i64(0);
    let start = offset.ge(zero.clone()).ite(
        &int_min(offset.clone(), total.clone()),
        &int_max(total.clone().add(offset.clone()), zero.clone()),
    );
    let remaining = total.sub(start);
    match length {
        Some(length) => length.ge(zero.clone()).ite(
            &int_min(length.clone(), remaining.clone()),
            &int_max(remaining.add(length.clone()), zero),
        ),
        None => remaining,
    }
}

fn builtin_length(tag: u64, kind: &str) -> Int {
    Int::new_const(format!("builtin::{kind}::{tag}::length"))
}

fn builtin_call_value(name: Option<&str>, arguments: &[Value], tag: u64) -> Option<Value> {
    let name = name?;
    match name {
        "strlen" => match arguments.first()? {
            Value::String(Some(value)) => Some(Value::Int(Int::from_u64(value.len() as u64))),
            Value::StringLength(length) => Some(Value::Int(length.clone())),
            _ => None,
        },
        "count" | "sizeof" => {
            if let Some(flag) = arguments.get(1) {
                if !matches!(flag, Value::Int(flag) if flag.as_i64() == Some(0)) {
                    return None;
                }
            }
            match arguments.first()? {
                Value::Array(Some(keys)) => Some(Value::Int(Int::from_u64(keys.len() as u64))),
                Value::ArrayLength(length) | Value::ListLength(length) => {
                    Some(Value::Int(length.clone()))
                }
                _ => None,
            }
        }
        "array_map" => {
            if arguments.len() < 2 {
                return None;
            }
            let arrays = &arguments[1..];
            let mut length = array_length(arrays.first()?)?;
            for array in &arrays[1..] {
                length = int_max(length, array_length(array)?);
            }
            if arrays.len() == 1 {
                match arrays.first()? {
                    Value::Array(Some(keys)) if !is_list_keys(keys) => {
                        Some(Value::ArrayLength(length))
                    }
                    Value::ArrayLength(_) => Some(Value::ArrayLength(length)),
                    _ => Some(Value::ListLength(length)),
                }
            } else {
                Some(Value::ListLength(length))
            }
        }
        "array_values" => match arguments.first()? {
            Value::Array(Some(keys)) => Some(Value::ListLength(Int::from_u64(keys.len() as u64))),
            Value::ArrayLength(length) | Value::ListLength(length) => {
                Some(Value::ListLength(length.clone()))
            }
            _ => None,
        },
        "array_reverse" => match arguments.first()? {
            Value::Array(Some(keys)) => {
                let length = Int::from_u64(keys.len() as u64);
                if is_list_keys(keys) {
                    Some(Value::ListLength(length))
                } else {
                    Some(Value::ArrayLength(length))
                }
            }
            Value::ArrayLength(length) => Some(Value::ArrayLength(length.clone())),
            Value::ListLength(length) => Some(Value::ListLength(length.clone())),
            _ => None,
        },
        "array_merge" => {
            if arguments.is_empty() {
                return Some(Value::ListLength(Int::from_i64(0)));
            }
            let mut total = Int::from_i64(0);
            for argument in arguments {
                let length = match argument {
                    Value::Array(Some(keys)) if is_list_keys(keys) => {
                        Int::from_u64(keys.len() as u64)
                    }
                    Value::ListLength(length) => length.clone(),
                    _ => return None,
                };
                total = total.add(length);
            }
            Some(Value::ListLength(total))
        }
        "array_diff" | "array_intersect" | "array_unique" | "array_filter" | "array_fill_keys" => {
            array_length(arguments.first()?)?;
            Some(Value::ArrayLength(builtin_length(tag, name)))
        }
        "array_keys" => {
            let length = array_length(arguments.first()?)?;
            if arguments.len() == 1 {
                Some(Value::ListLength(length))
            } else {
                Some(Value::ListLength(builtin_length(tag, name)))
            }
        }
        "array_fill" => {
            let count = match arguments.get(1)? {
                Value::Int(count) => count.clone(),
                _ => return None,
            };
            if matches!(arguments.first(), Some(Value::Int(start)) if start.as_i64() == Some(0)) {
                Some(Value::ListLength(count))
            } else {
                Some(Value::ArrayLength(count))
            }
        }
        "array_pad" => {
            let length = array_length(arguments.first()?)?;
            let size = match arguments.get(1)? {
                Value::Int(size) => size.clone(),
                _ => return None,
            };
            let magnitude = size.ge(Int::from_i64(0)).ite(&size, &size.unary_minus());
            let length = int_max(length, magnitude);
            let list = match arguments.first()? {
                Value::Array(Some(keys)) => is_list_keys(keys),
                Value::ListLength(_) => true,
                _ => false,
            };
            if list {
                Some(Value::ListLength(length))
            } else {
                Some(Value::ArrayLength(length))
            }
        }
        "array_slice" => {
            let total = array_length(arguments.first()?)?;
            let offset = match arguments.get(1)? {
                Value::Int(offset) => offset,
                _ => return None,
            };
            let requested = match arguments.get(2) {
                Some(Value::Int(length)) => Some(length),
                Some(Value::Null) | None => None,
                _ => return None,
            };
            let length = slice_length(total, offset, requested);
            let preserve_keys = match arguments.get(3) {
                None => false,
                Some(Value::Bool(value)) => value.as_bool().unwrap_or(true),
                _ => true,
            };
            let list = !preserve_keys
                && match arguments.first()? {
                    Value::Array(Some(keys)) => is_list_keys(keys),
                    Value::ListLength(_) => true,
                    _ => false,
                };
            if list {
                Some(Value::ListLength(length))
            } else {
                Some(Value::ArrayLength(length))
            }
        }
        "substr" => {
            let total = match arguments.first()? {
                Value::String(Some(value)) => Int::from_u64(value.len() as u64),
                Value::StringLength(length) => length.clone(),
                _ => return None,
            };
            let offset = match arguments.get(1)? {
                Value::Int(offset) => offset,
                _ => return None,
            };
            let requested = match arguments.get(2) {
                Some(Value::Int(length)) => Some(length),
                Some(Value::Null) | None => None,
                _ => return None,
            };
            Some(Value::StringLength(slice_length(total, offset, requested)))
        }
        "str_split" => {
            let total = match arguments.first()? {
                Value::String(Some(value)) => Int::from_u64(value.len() as u64),
                Value::StringLength(length) => length.clone(),
                _ => return None,
            };
            let size = match arguments.get(1) {
                Some(Value::Int(size)) => size.as_i64()?,
                None => 1,
                _ => return None,
            };
            if size <= 0 {
                return None;
            }
            Some(Value::ListLength(
                total.add(Int::from_i64(size - 1)).div(Int::from_i64(size)),
            ))
        }
        "str_pad" => {
            let total = match arguments.first()? {
                Value::String(Some(value)) => Int::from_u64(value.len() as u64),
                Value::StringLength(length) => length.clone(),
                _ => return None,
            };
            let target = match arguments.get(1)? {
                Value::Int(target) => target.clone(),
                _ => return None,
            };
            match arguments.get(2)? {
                Value::String(Some(padding)) if !padding.is_empty() => {}
                _ => return None,
            }
            Some(Value::StringLength(int_max(total, target)))
        }
        "array_chunk" => {
            let total = array_length(arguments.first()?)?;
            let size = match arguments.get(1)? {
                Value::Int(size) => size.as_i64()?,
                _ => return None,
            };
            if size <= 0 {
                return None;
            }
            Some(Value::ListLength(
                total.add(Int::from_i64(size - 1)).div(Int::from_i64(size)),
            ))
        }
        "str_repeat" => {
            let times = match arguments.get(1)? {
                Value::Int(times) => times.as_i64()?,
                _ => return None,
            };
            if times < 0 {
                return None;
            }
            let length = match arguments.first()? {
                Value::String(Some(value)) => Int::from_u64(value.len() as u64),
                Value::StringLength(length) => length.clone(),
                _ => return None,
            };
            Some(Value::StringLength(length.mul(Int::from_i64(times))))
        }
        "array_key_exists" => {
            let key = array_key(arguments.first()?)?;
            match arguments.get(1)? {
                Value::Array(Some(keys)) => Some(Value::Bool(Bool::from_bool(keys.contains(&key)))),
                Value::Array(None) => None,
                _ => None,
            }
        }
        "is_int" | "is_integer" | "is_long" => {
            type_predicate(arguments.first()?, |value| matches!(value, Value::Int(_)))
        }
        "is_float" | "is_double" => {
            type_predicate(arguments.first()?, |value| matches!(value, Value::Float(_)))
        }
        "is_string" => type_predicate(arguments.first()?, |value| {
            matches!(value, Value::String(_) | Value::StringLength(_))
        }),
        "is_bool" => type_predicate(arguments.first()?, |value| matches!(value, Value::Bool(_))),
        "is_array" => type_predicate(arguments.first()?, |value| {
            matches!(
                value,
                Value::Array(_) | Value::ArrayLength(_) | Value::ListLength(_)
            )
        }),
        "is_null" => type_predicate(arguments.first()?, |value| matches!(value, Value::Null)),
        _ => None,
    }
}

fn builtin_call_constraint(name: Option<&str>, arguments: &[Value], tag: u64) -> Option<Bool> {
    let name = name?;
    match name {
        "array_diff" | "array_intersect" | "array_unique" | "array_filter" | "array_fill_keys" => {
            let input = array_length(arguments.first()?)?;
            let length = builtin_length(tag, name);
            Some(Bool::and(&[
                &length.ge(Int::from_i64(0)),
                &length.le(input),
            ]))
        }
        "array_keys" if arguments.len() > 1 => {
            let input = array_length(arguments.first()?)?;
            let length = builtin_length(tag, name);
            Some(Bool::and(&[
                &length.ge(Int::from_i64(0)),
                &length.le(input),
            ]))
        }
        "array_fill" => match arguments.get(1)? {
            Value::Int(count) => Some(count.ge(Int::from_i64(0))),
            _ => None,
        },
        "array_slice" => {
            let total = array_length(arguments.first()?)?;
            let offset = match arguments.get(1)? {
                Value::Int(offset) => offset,
                _ => return None,
            };
            let length = match arguments.get(2) {
                Some(Value::Int(length)) => Some(length),
                Some(Value::Null) | None => None,
                _ => return None,
            };
            let length = slice_length(total.clone(), offset, length);
            Some(Bool::and(&[
                &length.ge(Int::from_i64(0)),
                &length.le(total),
            ]))
        }
        "substr" => {
            let total = match arguments.first()? {
                Value::String(Some(value)) => Int::from_u64(value.len() as u64),
                Value::StringLength(length) => length.clone(),
                _ => return None,
            };
            let offset = match arguments.get(1)? {
                Value::Int(offset) => offset,
                _ => return None,
            };
            let length = match arguments.get(2) {
                Some(Value::Int(length)) => Some(length),
                Some(Value::Null) | None => None,
                _ => return None,
            };
            let length = slice_length(total.clone(), offset, length);
            Some(Bool::and(&[
                &length.ge(Int::from_i64(0)),
                &length.le(total),
            ]))
        }
        "str_split" => {
            let total = match arguments.first()? {
                Value::String(Some(value)) => Int::from_u64(value.len() as u64),
                Value::StringLength(length) => length.clone(),
                _ => return None,
            };
            let size = match arguments.get(1) {
                Some(Value::Int(size)) => size.as_i64()?,
                None => 1,
                _ => return None,
            };
            if size <= 0 {
                return None;
            }
            let length = total
                .clone()
                .add(Int::from_i64(size - 1))
                .div(Int::from_i64(size));
            Some(Bool::and(&[
                &total.ge(Int::from_i64(0)),
                &length.ge(Int::from_i64(0)),
            ]))
        }
        "str_pad" => {
            let total = match arguments.first()? {
                Value::String(Some(value)) => Int::from_u64(value.len() as u64),
                Value::StringLength(length) => length.clone(),
                _ => return None,
            };
            match arguments.get(2)? {
                Value::String(Some(padding)) if !padding.is_empty() => {}
                _ => return None,
            }
            Some(total.ge(Int::from_i64(0)))
        }
        "array_chunk" => {
            let total = array_length(arguments.first()?)?;
            let size = match arguments.get(1)? {
                Value::Int(size) => size.as_i64()?,
                _ => return None,
            };
            if size <= 0 {
                return None;
            }
            Some(total.ge(Int::from_i64(0)))
        }
        _ => None,
    }
}

fn type_predicate(value: &Value, predicate: impl Fn(&Value) -> bool) -> Option<Value> {
    if matches!(
        value,
        Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. }
    ) {
        return None;
    }
    Some(Value::Bool(Bool::from_bool(predicate(value))))
}

impl<'a, 'arena> Analyzer<'a, 'arena> {
    fn parameter_object_value(&self, hint: Option<&Hint<'_>>) -> Option<Value> {
        let class = match hint? {
            Hint::Identifier(identifier) => {
                let namespace = self.namespace.clone().unwrap_or_default();
                let imports = self.imports.get(&namespace).cloned().unwrap_or_default();
                summary::resolve_name(identifier, self.namespace.as_deref(), &imports)
            }
            Hint::Self_(_) => self.class.clone()?,
            Hint::Parent(_) => {
                let class = self.class.as_ref()?;
                self.parents.get(class)?.clone()
            }
            _ => return None,
        };
        (!self.parents.values().any(|parent| parent == &class)).then_some(Value::Object(class))
    }
}

fn parameter_value(
    hint: Option<&Hint<'_>>,
    doc_type: Option<&str>,
    function: &str,
    parameter: &str,
) -> Value {
    if let Some(doc_type) = doc_type {
        let doc_type = doc_type.trim();
        if doc_type.starts_with('?') || doc_type.contains('|') || doc_type == "mixed" {
            return Value::Unknown;
        }
        if doc_type == "int"
            || doc_type == "integer"
            || doc_type.starts_with("int<")
            || matches!(
                doc_type,
                "positive-int"
                    | "non-negative-int"
                    | "negative-int"
                    | "non-positive-int"
                    | "non-zero-int"
            )
        {
            return Value::Int(Int::new_const(format!("{function}::{parameter}")));
        }
        if doc_type == "float" || doc_type == "double" {
            return Value::Float(Float::new_const(format!("{function}::{parameter}"), 11, 53));
        }
        if doc_type == "non-empty-string" {
            return Value::StringLength(Int::new_const(format!("{function}::{parameter}::length")));
        }
        if doc_type == "string" || doc_type == "numeric-string" {
            return Value::String(None);
        }
        if doc_type == "bool" || doc_type == "boolean" || doc_type == "true" || doc_type == "false"
        {
            return Value::Bool(Bool::new_const(format!("{function}::{parameter}")));
        }
        if doc_type == "non-empty-array" || doc_type.starts_with("non-empty-array<") {
            return Value::ArrayLength(Int::new_const(format!("{function}::{parameter}::length")));
        }
        if doc_type == "non-empty-list"
            || doc_type.starts_with("non-empty-list<")
            || doc_type == "list"
            || doc_type.starts_with("list<")
        {
            return Value::ListLength(Int::new_const(format!("{function}::{parameter}::length")));
        }
        if doc_type.starts_with("array{") {
            return Value::Array(contracts::array_shape_keys(doc_type));
        }
        if doc_type == "array" || doc_type.starts_with("array<") {
            return Value::ArrayLength(Int::new_const(format!("{function}::{parameter}::length")));
        }
        if doc_type == "iterable" {
            return Value::Array(None);
        }
        if doc_type == "null" {
            return Value::Null;
        }
        return Value::Unknown;
    }

    match hint {
        Some(Hint::Integer(_)) => Value::Int(Int::new_const(format!("{function}::{parameter}"))),
        Some(Hint::Mixed(_)) | None => Value::Unknown,
        Some(Hint::Float(_)) => {
            Value::Float(Float::new_const(format!("{function}::{parameter}"), 11, 53))
        }
        Some(Hint::String(_)) => Value::String(None),
        Some(Hint::Bool(_)) => Value::Bool(Bool::new_const(format!("{function}::{parameter}"))),
        Some(Hint::Array(_)) | Some(Hint::Iterable(_)) => Value::Array(None),
        Some(Hint::True(_)) => Value::Bool(Bool::from_bool(true)),
        Some(Hint::False(_)) => Value::Bool(Bool::from_bool(false)),
        Some(Hint::Null(_)) => Value::Null,
        _ => Value::Unknown,
    }
}

fn control_flow_level(level: Option<&Expression<'_>>) -> usize {
    let Some(level) = level else {
        return 1;
    };
    match level.unparenthesized() {
        Expression::Literal(Literal::Integer(integer)) => integer
            .value
            .and_then(|value| usize::try_from(value).ok())
            .filter(|level| *level > 0)
            .unwrap_or(1),
        _ => 1,
    }
}

fn arguments_are_positional(arguments: &ArgumentList<'_>) -> bool {
    arguments
        .arguments
        .iter()
        .all(|argument| matches!(argument, Argument::Positional(_)))
}

fn summary_call_value(
    summary: &FunctionSummary,
    arguments: &[Value],
    positional: bool,
    tag: u64,
) -> Option<(Value, Option<Bool>)> {
    let value = if let Some(value) = summary.return_value.as_ref() {
        constant_to_value(value)
    } else if positional {
        summary
            .return_parameter
            .and_then(|index| arguments.get(index))
            .cloned()
            .or_else(|| {
                summary
                    .return_contract
                    .as_deref()
                    .and_then(|ty| contract_value(ty, tag))
            })?
    } else {
        summary
            .return_contract
            .as_deref()
            .and_then(|ty| contract_value(ty, tag))?
    };
    let constraint = summary
        .return_contract
        .as_deref()
        .and_then(|ty| contracts::constraint(&value, ty));
    Some((value, constraint))
}

fn contract_value(ty: &str, tag: u64) -> Option<Value> {
    let ty = ty.trim();
    if ty.is_empty()
        || ty.starts_with('?')
        || ty.contains('|')
        || ty == "mixed"
        || ty.starts_with('(')
    {
        return None;
    }
    if ty.starts_with("int<")
        || matches!(
            ty,
            "positive-int"
                | "non-negative-int"
                | "negative-int"
                | "non-positive-int"
                | "non-zero-int"
        )
    {
        return Some(Value::Int(Int::new_const(format!("call::{tag}::return"))));
    }
    if ty == "non-empty-string" {
        return Some(Value::StringLength(Int::new_const(format!(
            "call::{tag}::return::length"
        ))));
    }
    if ty == "non-empty-array" || ty.starts_with("non-empty-array<") {
        return Some(Value::ArrayLength(Int::new_const(format!(
            "call::{tag}::return::length"
        ))));
    }
    if ty == "non-empty-list"
        || ty.starts_with("non-empty-list<")
        || ty == "list"
        || ty.starts_with("list<")
    {
        return Some(Value::ListLength(Int::new_const(format!(
            "call::{tag}::return::length"
        ))));
    }
    if ty == "array" || ty.starts_with("array<") {
        return Some(Value::ArrayLength(Int::new_const(format!(
            "call::{tag}::return::length"
        ))));
    }
    if ty.starts_with("array{") {
        return Some(Value::Array(contracts::array_shape_keys(ty)));
    }
    None
}

fn constant_to_value(value: &ConstantValue) -> Value {
    match value {
        ConstantValue::Int(value) => Value::Int(Int::from_i64(*value)),
        ConstantValue::Float(value) => Value::Float(Float::from_f64(*value)),
        ConstantValue::Bool(value) => Value::Bool(Bool::from_bool(*value)),
        ConstantValue::String(value) => Value::String(Some(value.clone())),
        ConstantValue::Null => Value::Null,
    }
}

fn is_pure_builtin(name: Option<&str>) -> bool {
    matches!(
        name,
        Some(
            "strlen"
                | "count"
                | "sizeof"
                | "array_key_exists"
                | "is_int"
                | "is_integer"
                | "is_long"
                | "is_float"
                | "is_double"
                | "is_string"
                | "is_bool"
                | "is_array"
                | "is_null"
                | "in_array"
                | "array_keys"
                | "array_key_first"
                | "array_key_last"
                | "array_values"
                | "array_search"
                | "array_sum"
                | "array_product"
                | "array_slice"
                | "array_merge"
                | "array_filter"
                | "array_map"
                | "array_reduce"
                | "array_unique"
                | "array_reverse"
                | "array_combine"
                | "array_diff"
                | "array_intersect"
                | "array_column"
                | "array_chunk"
                | "array_pad"
                | "array_fill"
                | "array_fill_keys"
                | "array_is_list"
                | "implode"
                | "join"
                | "explode"
                | "substr"
                | "str_split"
                | "str_pad"
        )
    )
}

fn is_resource_builtin(name: Option<&str>) -> bool {
    matches!(name, Some("fopen" | "fclose" | "fread" | "fwrite"))
}

fn is_fopen_expression(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Call(Call::Function(function)) => {
            function_name(function.function).as_deref() == Some("fopen")
        }
        Expression::UnaryPrefix(unary)
            if matches!(unary.operator, UnaryPrefixOperator::ErrorControl(_)) =>
        {
            is_fopen_expression(unary.operand)
        }
        _ => false,
    }
}

fn function_name(expression: &Expression<'_>) -> Option<String> {
    match expression.unparenthesized() {
        Expression::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.last_segment()).to_ascii_lowercase())
        }
        _ => None,
    }
}

fn direct_variable_name(expression: &Expression<'_>) -> Option<String> {
    match expression.unparenthesized() {
        Expression::Variable(Variable::Direct(variable)) => {
            Some(String::from_utf8_lossy(variable.name).to_string())
        }
        _ => None,
    }
}

fn root_variable_name(expression: &Expression<'_>) -> Option<String> {
    match expression.unparenthesized() {
        Expression::Variable(Variable::Direct(variable)) => {
            Some(String::from_utf8_lossy(variable.name).to_string())
        }
        Expression::ArrayAccess(access) => root_variable_name(access.array),
        Expression::Access(Access::Property(access)) => root_variable_name(access.object),
        Expression::Access(Access::NullSafeProperty(access)) => root_variable_name(access.object),
        _ => None,
    }
}

fn is_reference_expression(expression: &Expression<'_>) -> bool {
    matches!(
        expression.unparenthesized(),
        Expression::UnaryPrefix(unary)
            if matches!(unary.operator, UnaryPrefixOperator::Reference(_))
    )
}

fn foreach_reference_root(foreach: &Foreach<'_>) -> Option<String> {
    let reference = match &foreach.target {
        ForeachTarget::Value(target) => is_reference_expression(target.value),
        ForeachTarget::KeyValue(target) => {
            is_reference_expression(target.key) || is_reference_expression(target.value)
        }
    };
    reference
        .then(|| root_variable_name(foreach.expression))
        .flatten()
}

fn values_equal_strict(left: &Value, right: &Value) -> Option<bool> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => Some(left.as_i64()? == right.as_i64()?),
        (Value::Float(left), Value::Float(right)) => Some(left.as_f64() == right.as_f64()),
        (Value::Bool(left), Value::Bool(right)) => Some(left.as_bool()? == right.as_bool()?),
        (Value::String(Some(left)), Value::String(Some(right))) => Some(left == right),
        (Value::Null, Value::Null) => Some(true),
        (Value::Null, _) | (_, Value::Null) => Some(false),
        (Value::Int(_), Value::Float(_))
        | (Value::Float(_), Value::Int(_))
        | (Value::Bool(_), _)
        | (_, Value::Bool(_))
        | (Value::String(Some(_)), _)
        | (_, Value::String(Some(_))) => Some(false),
        _ => None,
    }
}

fn values_equal_loose(left: &Value, right: &Value) -> Option<bool> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => Some(left.as_i64()? == right.as_i64()?),
        (Value::Float(left), Value::Float(right)) => Some(left.as_f64() == right.as_f64()),
        (Value::Int(left), Value::Float(right)) => Some((left.as_i64()? as f64) == right.as_f64()),
        (Value::Float(left), Value::Int(right)) => Some(left.as_f64() == (right.as_i64()? as f64)),
        (Value::Bool(left), Value::Bool(right)) => Some(left.as_bool()? == right.as_bool()?),
        (Value::String(Some(left)), Value::String(Some(right))) => {
            match (left.parse::<f64>(), right.parse::<f64>()) {
                (Ok(left), Ok(right)) => Some(left == right),
                _ => Some(left == right),
            }
        }
        (Value::Null, Value::Null) => Some(true),
        _ => None,
    }
}

fn value_from_pairs(pairs: &[(Value, Value)]) -> Value {
    let mut keys = BTreeSet::new();
    for (key, _) in pairs {
        match array_key(key) {
            Some(key) => {
                keys.insert(key);
            }
            None => return Value::Array(None),
        }
    }
    Value::Array(Some(keys))
}

fn collect_and_operands<'a, 'arena>(
    expression: &'a Expression<'arena>,
    operands: &mut Vec<&'a Expression<'arena>>,
) {
    match expression.unparenthesized() {
        Expression::Binary(binary)
            if matches!(
                binary.operator,
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
            ) =>
        {
            collect_and_operands(binary.lhs, operands);
            collect_and_operands(binary.rhs, operands);
        }
        _ => operands.push(expression),
    }
}

fn path_not(value: &Bool) -> Bool {
    match value.as_bool() {
        Some(value) => Bool::from_bool(!value),
        None => value.not(),
    }
}

fn path_and(left: &Bool, right: &Bool) -> Bool {
    match (left.as_bool(), right.as_bool()) {
        (Some(false), _) | (_, Some(false)) => Bool::from_bool(false),
        (Some(true), Some(true)) => Bool::from_bool(true),
        (Some(true), None) => right.clone(),
        (None, Some(true)) => left.clone(),
        _ => Bool::and(&[left, right]),
    }
}

fn path_or_many(values: &[Bool]) -> Bool {
    if values.is_empty() {
        return Bool::from_bool(false);
    }

    if values.iter().any(|value| value.as_bool() == Some(true)) {
        return Bool::from_bool(true);
    }

    let values = values
        .iter()
        .filter(|value| value.as_bool() != Some(false))
        .collect::<Vec<_>>();

    match values.as_slice() {
        [] => Bool::from_bool(false),
        [value] => (*value).clone(),
        _ => Bool::or(&values),
    }
}

fn null_condition(value: &Value) -> Option<Bool> {
    match value {
        Value::Null => Some(Bool::from_bool(true)),
        Value::Unknown | Value::Unverified | Value::UnverifiedNumeric { .. } => None,
        _ => Some(Bool::from_bool(false)),
    }
}

fn path_may_be_true(path: &Bool) -> bool {
    !matches!(path.as_bool(), Some(false))
}

fn array_key(value: &Value) -> Option<ArrayKey> {
    match value {
        Value::Int(value) => value.as_i64().map(ArrayKey::Int),
        Value::Bool(value) => value
            .as_bool()
            .map(|value| ArrayKey::Int(if value { 1 } else { 0 })),
        Value::Float(value) => {
            let value = value.as_f64();
            if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
                return None;
            }
            Some(ArrayKey::Int(value.trunc() as i64))
        }
        Value::Null => Some(ArrayKey::String(String::new())),
        Value::String(Some(value)) => Some(normalize_array_key(value)),
        _ => None,
    }
}

fn normalize_array_key(value: &str) -> ArrayKey {
    if let Ok(index) = value.parse::<i64>() {
        if index.to_string() == value {
            return ArrayKey::Int(index);
        }
    }

    ArrayKey::String(value.to_string())
}

fn render_array_key(key: &ArrayKey) -> String {
    match key {
        ArrayKey::Int(value) => value.to_string(),
        ArrayKey::String(value) => format!("'{value}'"),
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Bool(_) => "bool",
        Value::String(_) | Value::StringLength(_) => "string",
        Value::Array(_) | Value::ArrayLength(_) | Value::ListLength(_) => "array",
        Value::Null => "null",
        Value::Unknown => "unknown",
        Value::Unverified | Value::UnverifiedNumeric { .. } => "unverified-number",
        Value::Object(_) => "object",
        Value::Resource { .. } => "resource",
    }
}

struct PrivateStaticCallCollector<'a> {
    class: &'a str,
    namespace: Option<&'a str>,
    imports: &'a HashMap<String, String>,
    parameters: &'a HashMap<String, Vec<String>>,
    allow_static_binding: bool,
    caller_parameters: &'a [String],
    caller_domains: &'a [Option<ParameterDomain>],
    domains: &'a HashMap<String, Vec<Option<ParameterDomain>>>,
    calls: &'a Mutex<HashMap<String, Vec<Vec<Option<ParameterDomain>>>>>,
    invalid: &'a Mutex<HashSet<String>>,
    inner_scope: &'a Mutex<usize>,
}

impl PrivateStaticCallCollector<'_> {
    fn invalidate(&self, method: &str) {
        if let Ok(mut invalid) = self.invalid.lock() {
            invalid.insert(method.to_string());
        }
    }

    fn invalidate_all(&self) {
        if let Ok(mut invalid) = self.invalid.lock() {
            invalid.extend(self.parameters.keys().cloned());
        }
    }

    fn enter_inner_scope(&self) {
        if let Ok(mut depth) = self.inner_scope.lock() {
            *depth += 1;
        }
    }

    fn exit_inner_scope(&self) {
        if let Ok(mut depth) = self.inner_scope.lock() {
            *depth = depth.saturating_sub(1);
        }
    }

    fn record(&self, method: &str, arguments: &ArgumentList<'_>) {
        if self
            .inner_scope
            .lock()
            .map(|depth| *depth > 0)
            .unwrap_or(true)
        {
            return;
        }
        let Some(parameters) = self.parameters.get(method) else {
            return;
        };
        let Some(values) = domain_arguments(
            arguments,
            parameters,
            self.caller_parameters,
            self.caller_domains,
            self.domains,
        ) else {
            self.invalidate(method);
            return;
        };
        if let Ok(mut calls) = self.calls.lock() {
            calls.entry(method.to_string()).or_default().push(values);
        }
    }

    fn is_current_class(&self, expression: &Expression<'_>) -> bool {
        match expression.unparenthesized() {
            Expression::Self_(_) => true,
            Expression::Static(_) if self.allow_static_binding => true,
            Expression::Identifier(identifier) => {
                summary::resolve_name(identifier, self.namespace, self.imports) == self.class
            }
            _ => false,
        }
    }

    fn is_this(&self, expression: &Expression<'_>) -> bool {
        direct_variable_name(expression).as_deref() == Some("$this")
    }
}

impl<'ast, 'arena> Walker<'ast, 'arena, ()> for PrivateStaticCallCollector<'_> {
    fn walk_closure(&self, closure: &'ast Closure<'arena>, context: &mut ()) {
        self.enter_inner_scope();
        mago_syntax::walker::walk_closure(self, closure, context);
        self.exit_inner_scope();
    }

    fn walk_arrow_function(&self, arrow: &'ast ArrowFunction<'arena>, context: &mut ()) {
        self.enter_inner_scope();
        mago_syntax::walker::walk_arrow_function(self, arrow, context);
        self.exit_inner_scope();
    }

    fn walk_anonymous_class(&self, class: &'ast AnonymousClass<'arena>, context: &mut ()) {
        self.enter_inner_scope();
        mago_syntax::walker::walk_anonymous_class(self, class, context);
        self.exit_inner_scope();
    }

    fn walk_static_method_call(&self, call: &'ast StaticMethodCall<'arena>, context: &mut ()) {
        if self.is_current_class(call.class) {
            match selector_identifier(&call.method) {
                Some(method) if self.parameters.contains_key(&method) => {
                    self.record(&method, &call.argument_list);
                }
                Some(_) => {}
                None => self.invalidate_all(),
            }
        }
        mago_syntax::walker::walk_static_method_call(self, call, context);
    }

    fn walk_method_call(&self, call: &'ast MethodCall<'arena>, context: &mut ()) {
        if self.is_this(call.object) {
            match selector_identifier(&call.method) {
                Some(method) if self.parameters.contains_key(&method) => {
                    self.record(&method, &call.argument_list);
                }
                Some(_) => {}
                None => self.invalidate_all(),
            }
        }
        mago_syntax::walker::walk_method_call(self, call, context);
    }

    fn walk_null_safe_method_call(&self, call: &'ast NullSafeMethodCall<'arena>, context: &mut ()) {
        if self.is_this(call.object) {
            match selector_identifier(&call.method) {
                Some(method) if self.parameters.contains_key(&method) => {
                    self.record(&method, &call.argument_list);
                }
                Some(_) => {}
                None => self.invalidate_all(),
            }
        }
        mago_syntax::walker::walk_null_safe_method_call(self, call, context);
    }

    fn walk_static_method_partial_application(
        &self,
        application: &'ast StaticMethodPartialApplication<'arena>,
        context: &mut (),
    ) {
        if self.is_current_class(application.class) {
            if let Some(method) = selector_identifier(&application.method) {
                if self.parameters.contains_key(&method) {
                    self.invalidate(&method);
                }
            } else {
                self.invalidate_all();
            }
        }
        mago_syntax::walker::walk_static_method_partial_application(self, application, context);
    }

    fn walk_method_partial_application(
        &self,
        application: &'ast MethodPartialApplication<'arena>,
        context: &mut (),
    ) {
        if self.is_this(application.object) {
            if let Some(method) = selector_identifier(&application.method) {
                if self.parameters.contains_key(&method) {
                    self.invalidate(&method);
                }
            } else {
                self.invalidate_all();
            }
        }
        mago_syntax::walker::walk_method_partial_application(self, application, context);
    }
}

fn infer_parameter_domains(
    calls: &[Vec<Option<ParameterDomain>>],
    arity: usize,
) -> Vec<Option<ParameterDomain>> {
    (0..arity)
        .map(|index| {
            let mut merged: Option<ParameterDomain> = None;
            for call in calls {
                let Some(Some(domain)) = call.get(index) else {
                    return None;
                };
                merged = match &merged {
                    Some(previous) => Some(merge_domains(previous, domain)?),
                    None => Some(domain.clone()),
                };
            }
            merged
        })
        .collect()
}

fn merge_domains(left: &ParameterDomain, right: &ParameterDomain) -> Option<ParameterDomain> {
    match (left, right) {
        (ParameterDomain::Finite(left), ParameterDomain::Finite(right)) => {
            if left == right {
                return Some(ParameterDomain::Finite(left.clone()));
            }
            let left = domain_int_bounds(&ParameterDomain::Finite(left.clone()))?;
            let right = domain_int_bounds(&ParameterDomain::Finite(right.clone()))?;
            Some(ParameterDomain::IntRange {
                min: left.0.min(right.0),
                max: left.1.max(right.1),
            })
        }
        (ParameterDomain::Finite(values), ParameterDomain::IntRange { min, max })
        | (ParameterDomain::IntRange { min, max }, ParameterDomain::Finite(values)) => {
            let (values_min, values_max) =
                domain_int_bounds(&ParameterDomain::Finite(values.clone()))?;
            Some(ParameterDomain::IntRange {
                min: values_min.min(*min),
                max: values_max.max(*max),
            })
        }
        (
            ParameterDomain::IntRange {
                min: left_min,
                max: left_max,
            },
            ParameterDomain::IntRange {
                min: right_min,
                max: right_max,
            },
        ) => Some(ParameterDomain::IntRange {
            min: (*left_min).min(*right_min),
            max: (*left_max).max(*right_max),
        }),
    }
}

fn domain_int_bounds(domain: &ParameterDomain) -> Option<(i64, i64)> {
    match domain {
        ParameterDomain::Finite(values) => {
            let mut min = i64::MAX;
            let mut max = i64::MIN;
            for value in values {
                let ConstantValue::Int(value) = value else {
                    return None;
                };
                min = min.min(*value);
                max = max.max(*value);
            }
            (max >= min).then_some((min, max))
        }
        ParameterDomain::IntRange { min, max } => (*max >= *min).then_some((*min, *max)),
    }
}

fn domain_parameter_value(
    function: &str,
    parameter: &str,
    domain: &ParameterDomain,
) -> Option<(Value, Option<Bool>)> {
    match domain {
        ParameterDomain::Finite(values) => finite_parameter_value(function, parameter, values),
        ParameterDomain::IntRange { min, max } if min == max => {
            Some((Value::Int(Int::from_i64(*min)), None))
        }
        ParameterDomain::IntRange { min, max } => {
            let variable = Int::new_const(format!("{function}::{parameter}::range"));
            let constraint = path_and(
                &variable.ge(Int::from_i64(*min)),
                &variable.le(Int::from_i64(*max)),
            );
            Some((Value::Int(variable), Some(constraint)))
        }
    }
}

fn domain_arguments(
    arguments: &ArgumentList<'_>,
    parameters: &[String],
    caller_parameters: &[String],
    caller_domains: &[Option<ParameterDomain>],
    domains: &HashMap<String, Vec<Option<ParameterDomain>>>,
) -> Option<Vec<Option<ParameterDomain>>> {
    let mut values = vec![None; parameters.len()];
    let mut assigned = vec![false; parameters.len()];
    let mut positional = 0;
    for argument in arguments.arguments.iter() {
        match argument {
            Argument::Positional(argument) if argument.ellipsis.is_none() => {
                if positional >= values.len() {
                    return None;
                }
                if assigned[positional] {
                    return None;
                }
                values[positional] = domain_from_expression(
                    argument.value,
                    caller_parameters,
                    caller_domains,
                    domains,
                );
                assigned[positional] = true;
                positional += 1;
            }
            Argument::Named(argument) => {
                let name = String::from_utf8_lossy(argument.name.value);
                let index = parameters
                    .iter()
                    .position(|parameter| parameter == name.as_ref())?;
                if assigned[index] {
                    return None;
                }
                values[index] = domain_from_expression(
                    argument.value,
                    caller_parameters,
                    caller_domains,
                    domains,
                );
                assigned[index] = true;
            }
            _ => return None,
        }
    }
    Some(values)
}

fn domain_from_expression(
    expression: &Expression<'_>,
    caller_parameters: &[String],
    caller_domains: &[Option<ParameterDomain>],
    domains: &HashMap<String, Vec<Option<ParameterDomain>>>,
) -> Option<ParameterDomain> {
    if let Some(value) = summary::constant_value(expression) {
        return Some(ParameterDomain::Finite(vec![value]));
    }

    match expression.unparenthesized() {
        Expression::Variable(Variable::Direct(variable)) => {
            let name = String::from_utf8_lossy(variable.name)
                .trim_start_matches('$')
                .to_string();
            let index = caller_parameters
                .iter()
                .position(|parameter| parameter == &name)?;
            caller_domains.get(index).cloned().flatten()
        }
        Expression::UnaryPrefix(unary) => {
            let operand =
                domain_from_expression(unary.operand, caller_parameters, caller_domains, domains)?;
            match unary.operator {
                UnaryPrefixOperator::Plus(_) => Some(operand),
                UnaryPrefixOperator::Negation(_) => negate_domain(&operand),
                _ => None,
            }
        }
        Expression::Binary(binary) => {
            let left =
                domain_from_expression(binary.lhs, caller_parameters, caller_domains, domains)?;
            let right =
                domain_from_expression(binary.rhs, caller_parameters, caller_domains, domains)?;
            binary_domain(binary.operator, &left, &right)
        }
        Expression::Call(Call::Function(function)) => {
            let name = function_name(function.function)?;
            builtin_domain(
                &name,
                &function.argument_list,
                caller_parameters,
                caller_domains,
                domains,
            )
        }
        _ => None,
    }
}

fn negate_domain(domain: &ParameterDomain) -> Option<ParameterDomain> {
    match domain {
        ParameterDomain::Finite(values) => {
            let mut values = values.clone();
            for value in &mut values {
                match value {
                    ConstantValue::Int(value) => *value = value.checked_neg()?,
                    ConstantValue::Float(value) => *value = -*value,
                    _ => return None,
                }
            }
            Some(ParameterDomain::Finite(values))
        }
        ParameterDomain::IntRange { min, max } => Some(ParameterDomain::IntRange {
            min: max.checked_neg()?,
            max: min.checked_neg()?,
        }),
    }
}

fn binary_domain(
    operator: BinaryOperator<'_>,
    left: &ParameterDomain,
    right: &ParameterDomain,
) -> Option<ParameterDomain> {
    let (left_min, left_max) = domain_int_bounds(left)?;
    let (right_min, right_max) = domain_int_bounds(right)?;
    let (min, max) = match operator {
        BinaryOperator::Addition(_) => (
            checked_range(left_min, 1, right_min, 1)?,
            checked_range(left_max, 1, right_max, 1)?,
        ),
        BinaryOperator::Subtraction(_) => (
            checked_range(left_min, 1, right_max, -1)?,
            checked_range(left_max, 1, right_min, -1)?,
        ),
        BinaryOperator::Multiplication(_) => {
            let products = [
                checked_range(left_min, right_min, 1, 1)?,
                checked_range(left_min, right_max, 1, 1)?,
                checked_range(left_max, right_min, 1, 1)?,
                checked_range(left_max, right_max, 1, 1)?,
            ];
            (*products.iter().min()?, *products.iter().max()?)
        }
        BinaryOperator::Modulo(_) if right_min == right_max && right_min > 0 => {
            (-(right_min - 1), right_min - 1)
        }
        _ => return None,
    };
    (min <= max).then_some(ParameterDomain::IntRange { min, max })
}

fn checked_range(left: i64, left_factor: i64, right: i64, right_factor: i64) -> Option<i64> {
    let value =
        i128::from(left) * i128::from(left_factor) + i128::from(right) * i128::from(right_factor);
    i64::try_from(value).ok()
}

fn builtin_domain(
    name: &str,
    arguments: &ArgumentList<'_>,
    caller_parameters: &[String],
    caller_domains: &[Option<ParameterDomain>],
    domains: &HashMap<String, Vec<Option<ParameterDomain>>>,
) -> Option<ParameterDomain> {
    let values = arguments
        .arguments
        .iter()
        .filter_map(|argument| match argument {
            Argument::Positional(argument) if argument.ellipsis.is_none() => {
                domain_from_expression(argument.value, caller_parameters, caller_domains, domains)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    match name {
        "max" | "min" => {
            if values.is_empty() || values.len() != arguments.arguments.len() {
                return None;
            }
            let bounds = values
                .iter()
                .map(domain_int_bounds)
                .collect::<Option<Vec<_>>>()?;
            let min = if name == "max" {
                bounds.iter().map(|(min, _)| *min).max()?
            } else {
                bounds.iter().map(|(min, _)| *min).min()?
            };
            let max = if name == "max" {
                bounds.iter().map(|(_, max)| *max).max()?
            } else {
                bounds.iter().map(|(_, max)| *max).min()?
            };
            Some(ParameterDomain::IntRange { min, max })
        }
        "abs" => {
            let (min, max) = domain_int_bounds(values.first()?)?;
            if min >= 0 {
                Some(ParameterDomain::IntRange { min, max })
            } else if max <= 0 {
                Some(ParameterDomain::IntRange {
                    min: max.checked_neg()?,
                    max: min.checked_neg()?,
                })
            } else {
                Some(ParameterDomain::IntRange {
                    min: 0,
                    max: min.checked_neg()?.max(max),
                })
            }
        }
        "intdiv" => {
            let (left_min, left_max) = domain_int_bounds(values.first()?)?;
            let (right_min, right_max) = domain_int_bounds(values.get(1)?)?;
            if right_min <= 0 && right_max >= 0 {
                return None;
            }
            let quotients = [
                left_min / right_min,
                left_min / right_max,
                left_max / right_min,
                left_max / right_max,
            ];
            Some(ParameterDomain::IntRange {
                min: *quotients.iter().min()?,
                max: *quotients.iter().max()?,
            })
        }
        "count" | "sizeof" | "strlen" | "mb_strlen" => Some(ParameterDomain::IntRange {
            min: 0,
            max: i64::MAX,
        }),
        _ => None,
    }
}

fn finite_parameter_value(
    function: &str,
    parameter: &str,
    values: &[ConstantValue],
) -> Option<(Value, Option<Bool>)> {
    if let [value] = values {
        return Some((constant_to_value(value), None));
    }
    if values.is_empty() || values.len() > 32 {
        return None;
    }
    if !values
        .iter()
        .all(|value| matches!(value, ConstantValue::Int(_)))
    {
        return None;
    }

    let variable = Int::new_const(format!("{function}::{parameter}::constant-set"));
    let conditions = values
        .iter()
        .map(|value| {
            let ConstantValue::Int(value) = value else {
                unreachable!();
            };
            variable.eq(Int::from_i64(*value))
        })
        .collect::<Vec<_>>();
    Some((
        Value::Int(variable),
        Some(path_or_many(conditions.as_slice())),
    ))
}

fn selector_identifier(selector: &ClassLikeMemberSelector<'_>) -> Option<String> {
    match selector {
        ClassLikeMemberSelector::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.value).to_ascii_lowercase())
        }
        _ => None,
    }
}

struct YieldFinder;

impl<'ast, 'arena> Walker<'ast, 'arena, bool> for YieldFinder {
    fn walk_yield(&self, _: &'ast Yield<'arena>, found: &mut bool) {
        *found = true;
    }

    fn walk_yield_from(&self, _: &'ast YieldFrom<'arena>, found: &mut bool) {
        *found = true;
    }
}

fn body_contains_yield(block: &Block<'_>) -> bool {
    let mut found = false;
    YieldFinder.walk_block(block, &mut found);
    found
}

fn hint_name(hint: &Hint<'_>) -> String {
    match hint {
        Hint::Identifier(identifier) => String::from_utf8_lossy(identifier.value()).into_owned(),
        Hint::Parenthesized(hint) => format!("({})", hint_name(hint.hint)),
        Hint::Nullable(hint) => format!("?{}", hint_name(hint.hint)),
        Hint::Union(hint) => format!("{}|{}", hint_name(hint.left), hint_name(hint.right)),
        Hint::Intersection(hint) => format!("{}&{}", hint_name(hint.left), hint_name(hint.right)),
        Hint::Null(_) => String::from("null"),
        Hint::True(_) => String::from("true"),
        Hint::False(_) => String::from("false"),
        Hint::Array(_) => String::from("array"),
        Hint::Callable(_) => String::from("callable"),
        Hint::Static(_) => String::from("static"),
        Hint::Self_(_) => String::from("self"),
        Hint::Parent(_) => String::from("parent"),
        Hint::Void(identifier)
        | Hint::Never(identifier)
        | Hint::Float(identifier)
        | Hint::Bool(identifier)
        | Hint::Integer(identifier)
        | Hint::String(identifier)
        | Hint::Object(identifier)
        | Hint::Mixed(identifier)
        | Hint::Iterable(identifier) => String::from_utf8_lossy(identifier.value).into_owned(),
    }
}

fn hint_accepts_value(value: &Value, hint: &Hint<'_>, strict: bool) -> Option<bool> {
    match hint {
        Hint::Nullable(hint) => {
            if matches!(value, Value::Null) {
                Some(true)
            } else {
                hint_accepts_value(value, hint.hint, strict)
            }
        }
        Hint::Union(union) => match (
            hint_accepts_value(value, union.left, strict),
            hint_accepts_value(value, union.right, strict),
        ) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        Hint::Parenthesized(hint) => hint_accepts_value(value, hint.hint, strict),
        Hint::Mixed(_) => Some(true),
        Hint::Null(_) => Some(matches!(value, Value::Null)),
        Hint::True(_) => match value {
            Value::Bool(value) => match value.as_bool() {
                Some(true) => Some(true),
                Some(false) => Some(false),
                None => None,
            },
            _ => Some(false),
        },
        Hint::False(_) => match value {
            Value::Bool(value) => match value.as_bool() {
                Some(true) => Some(false),
                Some(false) => Some(true),
                None => None,
            },
            _ => Some(false),
        },
        Hint::Array(_) => Some(matches!(
            value,
            Value::Array(_) | Value::ArrayLength(_) | Value::ListLength(_)
        )),
        Hint::Callable(_) => Some(false),
        Hint::Integer(_) => match value {
            Value::Int(_) => Some(true),
            Value::Float(_) => {
                if strict {
                    Some(false)
                } else {
                    None
                }
            }
            Value::Bool(_) => Some(!strict),
            Value::String(Some(value)) => {
                if strict {
                    Some(false)
                } else {
                    Some(value.parse::<i64>().is_ok())
                }
            }
            Value::String(None) | Value::StringLength(_) => None,
            _ => Some(false),
        },
        Hint::Float(_) => match value {
            Value::Float(_) | Value::Int(_) => Some(true),
            Value::Bool(_) => Some(!strict),
            Value::String(Some(value)) => {
                if strict {
                    Some(false)
                } else {
                    Some(value.parse::<f64>().is_ok())
                }
            }
            Value::String(None) | Value::StringLength(_) => None,
            _ => Some(false),
        },
        Hint::String(_) => match value {
            Value::String(_) | Value::StringLength(_) => Some(true),
            Value::Int(_) | Value::Float(_) | Value::Bool(_) => Some(!strict),
            _ => Some(false),
        },
        Hint::Bool(_) => match value {
            Value::Bool(_) => Some(true),
            Value::Int(_) | Value::Float(_) | Value::String(_) | Value::StringLength(_) => {
                Some(!strict)
            }
            _ => Some(false),
        },
        Hint::Void(_) | Hint::Never(_) => Some(false),
        Hint::Iterable(_) => Some(matches!(
            value,
            Value::Array(_) | Value::ArrayLength(_) | Value::ListLength(_)
        )),
        Hint::Object(_)
        | Hint::Identifier(_)
        | Hint::Intersection(_)
        | Hint::Self_(_)
        | Hint::Parent(_)
        | Hint::Static(_) => Some(false),
    }
}

fn is_declaration(statement: &Statement<'_>) -> bool {
    matches!(
        statement,
        Statement::Function(_)
            | Statement::Class(_)
            | Statement::Interface(_)
            | Statement::Trait(_)
            | Statement::Enum(_)
    )
}

fn file_declares_strict_types(program: &Program<'_>) -> bool {
    statements_declare_strict_types(program.statements.as_slice())
}

fn statements_declare_strict_types(statements: &[Statement<'_>]) -> bool {
    for statement in statements {
        match statement {
            Statement::Declare(declare) => {
                for item in declare.items.iter() {
                    if String::from_utf8_lossy(item.name.value) != "strict_types" {
                        continue;
                    }
                    match item.value.unparenthesized() {
                        Expression::Literal(Literal::Integer(integer))
                            if integer.value == Some(1) =>
                        {
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            Statement::Namespace(namespace)
                if statements_declare_strict_types(namespace.statements().as_slice()) =>
            {
                return true;
            }
            _ => {}
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    fn keys(values: &[i64]) -> BTreeSet<ArrayKey> {
        values.iter().copied().map(ArrayKey::Int).collect()
    }

    #[test]
    fn array_slice_requires_a_known_false_preserve_keys_flag() {
        let array = Value::Array(Some(keys(&[0, 1, 2])));
        let arguments = [
            array.clone(),
            Value::Int(Int::from_i64(1)),
            Value::Int(Int::from_i64(2)),
        ];
        assert!(matches!(
            builtin_call_value(Some("array_slice"), &arguments, 1),
            Some(Value::ListLength(_))
        ));

        let mut with_false = arguments.to_vec();
        with_false.push(Value::Bool(Bool::from_bool(false)));
        assert!(matches!(
            builtin_call_value(Some("array_slice"), &with_false, 2),
            Some(Value::ListLength(_))
        ));

        let mut with_true = arguments.to_vec();
        with_true.push(Value::Bool(Bool::from_bool(true)));
        assert!(matches!(
            builtin_call_value(Some("array_slice"), &with_true, 3),
            Some(Value::ArrayLength(_))
        ));

        let mut with_unknown = arguments.to_vec();
        with_unknown.push(Value::Unknown);
        assert!(matches!(
            builtin_call_value(Some("array_slice"), &with_unknown, 4),
            Some(Value::ArrayLength(_))
        ));
    }

    #[test]
    fn count_with_a_recursive_flag_is_not_modeled() {
        let arguments = [
            Value::Array(Some(keys(&[0, 1, 2]))),
            Value::Int(Int::from_i64(1)),
        ];
        assert!(builtin_call_value(Some("count"), &arguments, 1).is_none());

        let normal = [
            Value::Array(Some(keys(&[0, 1, 2]))),
            Value::Int(Int::from_i64(0)),
        ];
        assert!(matches!(
            builtin_call_value(Some("count"), &normal, 2),
            Some(Value::Int(_))
        ));
    }
}
