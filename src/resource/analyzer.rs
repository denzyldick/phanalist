#![allow(clippy::field_reassign_with_default)]

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;
use serde::{Deserialize, Serialize};

use crate::file::File;
use crate::results::Message;

use super::model::{CostExpr, Growth, HeapEffect, SizeExpr, ValueState};
use super::summary::ResourceSummary;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResourceClass {
    RuntimeGrowth,
    LiveHeap,
    InputCardinality,
}

impl ResourceClass {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RuntimeGrowth => "E0044",
            Self::LiveHeap => "E0045",
            Self::InputCardinality => "E0046",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::RuntimeGrowth => "Input-dependent runtime growth",
            Self::LiveHeap => "Input-dependent live-heap growth",
            Self::InputCardinality => "Input-controlled cardinality",
        }
    }

    pub const fn all() -> [Self; 3] {
        [Self::RuntimeGrowth, Self::LiveHeap, Self::InputCardinality]
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ResourceSettings {
    pub runtime_threshold: String,
    pub heap_alloc_threshold: String,
    pub retained_element_bytes: u64,
    pub report_input_cardinality: bool,
    pub include_cli_input: bool,
    pub max_findings_per_function: usize,
}

impl Default for ResourceSettings {
    fn default() -> Self {
        Self {
            runtime_threshold: String::from("quadratic"),
            heap_alloc_threshold: String::from("quadratic"),
            retained_element_bytes: 32,
            report_input_cardinality: true,
            include_cli_input: true,
            max_findings_per_function: 20,
        }
    }
}

impl ResourceSettings {
    fn runtime_degree(&self) -> u8 {
        threshold_degree(&self.runtime_threshold)
    }

    fn heap_degree(&self) -> u8 {
        threshold_degree(&self.heap_alloc_threshold)
    }
}

fn threshold_degree(value: &str) -> u8 {
    match value.to_ascii_lowercase().as_str() {
        "linear" => 1,
        "quadratic" => 2,
        "cubic" => 3,
        "exponential" => u8::MAX,
        _ => 2,
    }
}

#[derive(Clone, Debug)]
pub struct ResourceFinding {
    pub class: ResourceClass,
    pub message: Message,
    pub span: Span,
}

#[derive(Clone, Debug)]
struct Eval {
    value: ValueState,
    effect: Effect,
}

impl Eval {
    fn pure(value: ValueState) -> Self {
        Self {
            value,
            effect: Effect::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Effect {
    cost: CostExpr,
    alloc: CostExpr,
    retained_delta: SizeExpr,
    peak: SizeExpr,
    tainted: bool,
    origins: BTreeSet<String>,
    growth: HashMap<String, Growth>,
    retains: bool,
}

impl Default for SizeExpr {
    fn default() -> Self {
        SizeExpr::Const(0)
    }
}

impl Effect {
    fn add(self, other: Self) -> Self {
        let mut growth = self.growth;
        for (name, value) in other.growth {
            growth
                .entry(name)
                .and_modify(|existing| {
                    *existing = Growth {
                        kind: existing.kind,
                        count_per_iteration: existing
                            .count_per_iteration
                            .clone()
                            .add(value.count_per_iteration.clone()),
                        bytes_per_iteration: existing
                            .bytes_per_iteration
                            .clone()
                            .add(value.bytes_per_iteration.clone()),
                    };
                })
                .or_insert(value);
        }
        Self {
            cost: self.cost.add(other.cost),
            alloc: self.alloc.add(other.alloc),
            retained_delta: self.retained_delta.add(other.retained_delta),
            peak: self.peak.max(other.peak),
            tainted: self.tainted || other.tainted,
            origins: self.origins.union(&other.origins).cloned().collect(),
            growth,
            retains: self.retains || other.retains,
        }
    }

    fn branch_merge(self, other: Self) -> Self {
        let mut growth = self.growth;
        growth.extend(other.growth);
        Self {
            cost: self.cost.add(other.cost),
            alloc: self.alloc.add(other.alloc),
            retained_delta: self.retained_delta.max(other.retained_delta),
            peak: self.peak.max(other.peak),
            tainted: self.tainted || other.tainted,
            origins: self.origins.union(&other.origins).cloned().collect(),
            growth,
            retains: self.retains || other.retains,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct State {
    env: HashMap<String, ValueState>,
    caps: HashMap<String, SizeExpr>,
}

impl State {
    fn with_caps(&self, guards: &[Guard]) -> Self {
        let mut state = self.clone();
        for guard in guards {
            state
                .caps
                .entry(guard.name.clone())
                .and_modify(|existing| {
                    *existing = existing.clone().min(guard.cap.clone());
                })
                .or_insert_with(|| guard.cap.clone());
        }
        state
    }

    fn merge(left: &Self, right: &Self) -> Self {
        let mut env = left.env.clone();
        for (name, value) in &right.env {
            env.entry(name.clone())
                .and_modify(|existing| {
                    *existing = ValueState::merge(existing, value);
                })
                .or_insert_with(|| value.clone());
        }
        let mut caps = left.caps.clone();
        for (name, cap) in &right.caps {
            caps.entry(name.clone())
                .and_modify(|existing| {
                    *existing = existing.clone().max(cap.clone());
                })
                .or_insert_with(|| cap.clone());
        }
        Self { env, caps }
    }
}

#[derive(Clone, Debug)]
struct Guard {
    name: String,
    cap: SizeExpr,
}

pub fn analyze(
    file: &File<'_>,
    settings: &ResourceSettings,
    classes: &[ResourceClass],
    summaries: &HashMap<String, ResourceSummary>,
) -> Vec<ResourceFinding> {
    if classes.is_empty() {
        return Vec::new();
    }

    let Some(program) = file.ast else {
        return Vec::new();
    };

    let mut findings = Vec::new();
    collect_statement_findings(
        file,
        program.statements.as_slice(),
        settings,
        classes,
        summaries,
        &mut findings,
    );

    let mut seen = BTreeSet::new();
    findings.retain(|finding| {
        seen.insert((
            finding.class,
            finding.span.start.offset,
            finding.span.end.offset,
            finding.message.id.clone(),
        ))
    });
    findings
}

fn collect_statement_findings(
    _file: &File<'_>,
    statements: &[Statement<'_>],
    settings: &ResourceSettings,
    classes: &[ResourceClass],
    summaries: &HashMap<String, ResourceSummary>,
    findings: &mut Vec<ResourceFinding>,
) {
    for statement in statements {
        match statement {
            Statement::Namespace(namespace) => collect_statement_findings(
                _file,
                namespace.statements().as_slice(),
                settings,
                classes,
                summaries,
                findings,
            ),
            Statement::Function(function) => {
                let params = parameter_names(&function.parameter_list);
                let mut analyzer = Analyzer::new(settings, classes, summaries, &params, false);
                analyzer.analyze_body(function.body.statements.as_slice());
                findings.extend(analyzer.findings);
            }
            Statement::Class(class) => collect_member_findings(
                &String::from_utf8_lossy(class.name.value),
                class.members.iter(),
                settings,
                classes,
                summaries,
                findings,
            ),
            Statement::Interface(interface) => collect_member_findings(
                &String::from_utf8_lossy(interface.name.value),
                interface.members.iter(),
                settings,
                classes,
                summaries,
                findings,
            ),
            Statement::Trait(r#trait) => collect_member_findings(
                &String::from_utf8_lossy(r#trait.name.value),
                r#trait.members.iter(),
                settings,
                classes,
                summaries,
                findings,
            ),
            Statement::Enum(r#enum) => collect_member_findings(
                &String::from_utf8_lossy(r#enum.name.value),
                r#enum.members.iter(),
                settings,
                classes,
                summaries,
                findings,
            ),
            Statement::Block(block) => collect_statement_findings(
                _file,
                block.statements.as_slice(),
                settings,
                classes,
                summaries,
                findings,
            ),
            _ => {}
        }
    }

    let mut top_level = Analyzer::new(settings, classes, summaries, &[], false);
    top_level.analyze_body(statements);
    findings.extend(top_level.findings);
}

fn collect_member_findings<'arena>(
    _class_name: &str,
    members: impl Iterator<Item = &'arena ClassLikeMember<'arena>>,
    settings: &ResourceSettings,
    classes: &[ResourceClass],
    summaries: &HashMap<String, ResourceSummary>,
    findings: &mut Vec<ResourceFinding>,
) {
    for member in members {
        let ClassLikeMember::Method(method) = member else {
            continue;
        };
        let MethodBody::Concrete(body) = &method.body else {
            continue;
        };
        let params = parameter_names(&method.parameter_list);
        let mut analyzer = Analyzer::new(settings, classes, summaries, &params, false);
        analyzer.analyze_body(body.statements.as_slice());
        findings.extend(analyzer.findings);
    }
}

fn parameter_names(parameter_list: &FunctionLikeParameterList<'_>) -> Vec<String> {
    parameter_list
        .parameters
        .iter()
        .map(|parameter| String::from_utf8_lossy(parameter.variable.name).to_string())
        .collect()
}

pub fn summarize_function(
    _file: &File<'_>,
    _name: &str,
    params: &[String],
    statements: &[Statement<'_>],
) -> Option<ResourceSummary> {
    let settings = ResourceSettings::default();
    let empty = HashMap::new();
    let mut analyzer = Analyzer::new(&settings, &[], &empty, params, true);
    analyzer.analyze_body(statements);
    Some(analyzer.finish_summary(params))
}

struct Analyzer<'a> {
    settings: &'a ResourceSettings,
    classes: &'a [ResourceClass],
    summaries: &'a HashMap<String, ResourceSummary>,
    params: Vec<String>,
    summary_mode: bool,
    findings: Vec<ResourceFinding>,
    total_cost: CostExpr,
    total_alloc: CostExpr,
    total_peak: SizeExpr,
    return_value: Option<ValueState>,
    sensitive_params: BTreeSet<String>,
    recursive: bool,
    loop_depth: usize,
    expr_depth: usize,
    size_depth: Cell<usize>,
    taint_depth: Cell<usize>,
}

impl<'a> Analyzer<'a> {
    fn new(
        settings: &'a ResourceSettings,
        classes: &'a [ResourceClass],
        summaries: &'a HashMap<String, ResourceSummary>,
        params: &[String],
        summary_mode: bool,
    ) -> Self {
        Self {
            settings,
            classes,
            summaries,
            params: params.to_vec(),
            summary_mode,
            findings: Vec::new(),
            total_cost: CostExpr::Constant,
            total_alloc: CostExpr::Constant,
            total_peak: SizeExpr::Const(0),
            return_value: None,
            sensitive_params: BTreeSet::new(),
            recursive: false,
            loop_depth: 0,
            expr_depth: 0,
            size_depth: Cell::new(0),
            taint_depth: Cell::new(0),
        }
    }

    fn analyze_body(&mut self, statements: &[Statement<'_>]) {
        let mut state = State::default();
        for param in &self.params {
            state.env.insert(param.clone(), ValueState::param(param));
        }
        let effect = self.analyze_statements(statements, &mut state);
        self.total_cost = self.total_cost.clone().add(effect.cost);
        self.total_alloc = self.total_alloc.clone().add(effect.alloc);
        self.total_peak = self.total_peak.clone().max(effect.peak);
    }

    fn finish_summary(&self, params: &[String]) -> ResourceSummary {
        let retained = self
            .return_value
            .as_ref()
            .map(|value| value.bytes.clone())
            .unwrap_or_else(|| SizeExpr::Const(0));
        let peak = self.total_peak.clone().max(retained.clone());
        let return_tainted_from = self
            .return_value
            .as_ref()
            .map(|value| {
                value
                    .taint_origins
                    .iter()
                    .filter_map(|origin| params.iter().position(|param| param == origin))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ResourceSummary {
            params: params.to_vec(),
            cost: self.total_cost.clone(),
            heap: HeapEffect {
                alloc: self.total_alloc.clone(),
                retained,
                peak,
                tainted: self
                    .return_value
                    .as_ref()
                    .is_some_and(|value| value.tainted),
            },
            return_count: self
                .return_value
                .as_ref()
                .map(|value| value.count.clone())
                .unwrap_or(SizeExpr::Unknown),
            return_bytes: self
                .return_value
                .as_ref()
                .map(|value| value.bytes.clone())
                .unwrap_or(SizeExpr::Unknown),
            return_tainted_from,
            sensitive_params: self.sensitive_params.iter().cloned().collect(),
            recursive: self.recursive,
        }
    }

    fn analyze_statements(&mut self, statements: &[Statement<'_>], state: &mut State) -> Effect {
        let mut effect = Effect::default();
        for statement in statements {
            let statement_effect = self.analyze_statement(statement, state);
            effect = effect.add(statement_effect);
        }
        effect
    }

    fn analyze_statement(&mut self, statement: &Statement<'_>, state: &mut State) -> Effect {
        match statement {
            Statement::Expression(expression) => {
                self.eval_expr(expression.expression, state).effect
            }
            Statement::Echo(echo) => {
                let mut effect = Effect::default();
                for value in echo.values.iter() {
                    effect = effect.add(self.eval_expr(value, state).effect);
                }
                effect
            }
            Statement::EchoTag(echo) => {
                let mut effect = Effect::default();
                for value in echo.values.iter() {
                    effect = effect.add(self.eval_expr(value, state).effect);
                }
                effect
            }
            Statement::Return(return_statement) => {
                let mut effect = Effect::default();
                if let Some(value) = return_statement.value {
                    let evaluated = self.eval_expr(value, state);
                    self.return_value = Some(evaluated.value.clone());
                    effect = effect.add(evaluated.effect);
                }
                effect
            }
            Statement::If(if_statement) => self.analyze_if(if_statement, state),
            Statement::Foreach(foreach) => self.analyze_foreach(foreach, state),
            Statement::For(for_statement) => self.analyze_for(for_statement, state),
            Statement::While(while_statement) => self.analyze_while(while_statement, state),
            Statement::DoWhile(do_while) => self.analyze_do_while(do_while, state),
            Statement::Try(try_statement) => {
                let mut effect =
                    self.analyze_statements(try_statement.block.statements.as_slice(), state);
                for catch in try_statement.catch_clauses.iter() {
                    let mut catch_state = state.clone();
                    if let Some(variable) = &catch.variable {
                        let name = String::from_utf8_lossy(variable.name).to_string();
                        catch_state.env.insert(name, ValueState::default());
                    }
                    effect =
                        effect.branch_merge(self.analyze_statements(
                            catch.block.statements.as_slice(),
                            &mut catch_state,
                        ));
                }
                if let Some(finally) = &try_statement.finally_clause {
                    effect = effect
                        .add(self.analyze_statements(finally.block.statements.as_slice(), state));
                }
                effect
            }
            Statement::Switch(switch_statement) => {
                let mut effect = Effect::default();
                for case in switch_statement.body.cases().iter() {
                    let mut case_state = state.clone();
                    effect = effect
                        .branch_merge(self.analyze_statements(case.statements(), &mut case_state));
                }
                effect
            }
            Statement::Block(block) => self.analyze_statements(block.statements.as_slice(), state),
            Statement::Namespace(namespace) => {
                self.analyze_statements(namespace.statements().as_slice(), state)
            }
            Statement::Unset(unset) => {
                for value in unset.values.iter() {
                    if let Some(name) = direct_variable_name(value) {
                        state.env.remove(name);
                        state.caps.remove(name);
                    }
                }
                Effect::default()
            }
            _ => Effect::default(),
        }
    }

    fn analyze_if(&mut self, if_statement: &If<'_>, state: &mut State) -> Effect {
        let condition = self.eval_expr(if_statement.condition, state);
        let then_guards = self.guards_from_condition(if_statement.condition, true, state);
        let else_guards = self.guards_from_condition(if_statement.condition, false, state);

        let mut then_state = state.clone().with_caps(&then_guards);
        let then_effect = self.analyze_statements(if_statement.body.statements(), &mut then_state);

        let else_statements = if_statement.body.else_statements();
        let else_effect = if let Some(else_statements) = else_statements {
            let mut else_state = state.clone().with_caps(&else_guards);
            self.analyze_statements(else_statements, &mut else_state)
        } else {
            Effect::default()
        };

        let then_exits = statements_always_exit(if_statement.body.statements());
        let else_exits = else_statements.is_some_and(statements_always_exit);

        if then_exits && !else_exits {
            *state = state.clone().with_caps(&else_guards);
        } else if else_exits && !then_exits {
            *state = then_state;
        } else if !then_exits && !else_exits {
            *state = State::merge(&then_state, &else_state_from(state, &else_guards));
        }

        let mut effect = condition.effect.add(then_effect.branch_merge(else_effect));
        for clause in if_statement.body.else_if_clauses() {
            let (condition, statements) = clause;
            let condition_eval = self.eval_expr(condition, state);
            let guards = self.guards_from_condition(condition, true, state);
            let mut clause_state = state.clone().with_caps(&guards);
            let clause_effect = self.analyze_statements(statements, &mut clause_state);
            effect = effect.add(condition_eval.effect.branch_merge(clause_effect));
        }
        effect
    }

    fn analyze_foreach(&mut self, foreach: &Foreach<'_>, state: &mut State) -> Effect {
        let iterable = self.eval_expr(foreach.expression, state);
        let bound = self.size_of_expr(foreach.expression, state);
        let origins = self.taint_of_expr(foreach.expression, state);
        let tainted = !origins.is_empty() || iterable.value.tainted;
        self.analyze_loop(
            foreach.span(),
            bound,
            tainted,
            origins,
            foreach.body.statements(),
            state,
            Some(foreach),
        )
    }

    fn analyze_for(&mut self, for_statement: &For<'_>, state: &mut State) -> Effect {
        let mut effect = Effect::default();
        for initialization in for_statement.initializations.iter() {
            effect = effect.add(self.eval_expr(initialization, state).effect);
        }
        let bound = for_statement
            .conditions
            .iter()
            .next()
            .map(|condition| self.loop_bound_from_condition(condition, state))
            .unwrap_or(SizeExpr::Unknown);
        let origins = for_statement
            .conditions
            .iter()
            .next()
            .map(|condition| self.taint_of_expr(condition, state))
            .unwrap_or_default();
        let loop_effect = self.analyze_loop(
            for_statement.span(),
            bound,
            !origins.is_empty(),
            origins,
            for_statement.body.statements(),
            state,
            None,
        );
        effect.add(loop_effect)
    }

    fn analyze_while(&mut self, while_statement: &While<'_>, state: &mut State) -> Effect {
        let condition = self.eval_expr(while_statement.condition, state);
        let bound = self.loop_bound_from_condition(while_statement.condition, state);
        let origins = self.taint_of_expr(while_statement.condition, state);
        condition.effect.add(self.analyze_loop(
            while_statement.span(),
            bound,
            !origins.is_empty(),
            origins,
            while_statement.body.statements(),
            state,
            None,
        ))
    }

    fn analyze_do_while(&mut self, do_while: &DoWhile<'_>, state: &mut State) -> Effect {
        let bound = self.loop_bound_from_condition(do_while.condition, state);
        let origins = self.taint_of_expr(do_while.condition, state);
        let body = std::slice::from_ref(do_while.statement);
        self.analyze_loop(
            do_while.span(),
            bound,
            !origins.is_empty(),
            origins,
            body,
            state,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn analyze_loop(
        &mut self,
        span: Span,
        bound: SizeExpr,
        tainted: bool,
        origins: BTreeSet<String>,
        statements: &[Statement<'_>],
        state: &mut State,
        foreach: Option<&Foreach<'_>>,
    ) -> Effect {
        self.loop_depth += 1;
        let pre_env = state.env.clone();
        let base_retained = sum_bytes(&pre_env);
        let mut body_state = state.clone();

        if let Some(foreach) = foreach {
            let element = self.element_state(foreach.expression, state);
            if let Some(name) = direct_variable_name(foreach.target.value()) {
                body_state.env.insert(name.to_string(), element.clone());
            }
            if let Some(key) = foreach.target.key() {
                if let Some(name) = direct_variable_name(key) {
                    body_state
                        .env
                        .insert(name.to_string(), ValueState::scalar());
                }
            }
        }

        let body_effect = self.analyze_statements(statements, &mut body_state);
        self.loop_depth -= 1;

        let iterations = bound.clone();
        let mut loop_cost = body_effect.cost.clone().multiply_by(iterations.clone());
        let mut loop_alloc = body_effect.alloc.clone().multiply_by(iterations.clone());
        let mut loop_retained = base_retained.clone();
        let mut loop_peak = body_effect.peak.clone().max(base_retained.clone());
        let mut copy_growth = false;

        for (name, growth) in &body_effect.growth {
            let Some(base) = pre_env.get(name) else {
                continue;
            };
            let added_count = growth.count_per_iteration.clone().mul(iterations.clone());
            let added_bytes = growth.bytes_per_iteration.clone().mul(iterations.clone());
            loop_retained = loop_retained.clone().add(added_bytes.clone());
            loop_peak = loop_peak.clone().max(loop_retained.clone());
            if growth.is_copying() {
                copy_growth = true;
                loop_cost = loop_cost.add(CostExpr::quadratic(iterations.clone()));
                loop_alloc = loop_alloc.add(CostExpr::linear(
                    growth
                        .bytes_per_iteration
                        .clone()
                        .mul(iterations.clone())
                        .mul(iterations.clone()),
                ));
            }
            if let Some(entry) = state.env.get_mut(name) {
                entry.count = base.count.clone().add(added_count);
                entry.bytes = base.bytes.clone().add(added_bytes);
                entry.alloc = entry.alloc.clone().add(CostExpr::linear(
                    growth.bytes_per_iteration.clone().mul(iterations.clone()),
                ));
                entry.peak = base.peak.clone().max(entry.bytes.clone());
                entry.tainted = base.tainted || tainted;
                entry.taint_origins = base.taint_origins.union(&origins).cloned().collect();
                entry.growth = None;
            }
        }

        if body_effect.growth.is_empty() && body_effect.retained_delta != SizeExpr::Const(0) {
            let added = body_effect.retained_delta.clone().mul(iterations.clone());
            loop_retained = loop_retained.clone().add(added.clone());
            loop_peak = loop_peak.clone().max(loop_retained.clone());
        }

        for name in body_state.env.keys() {
            if body_effect.growth.contains_key(name) {
                continue;
            }
            if pre_env.contains_key(name) {
                if let Some(previous) = pre_env.get(name) {
                    state.env.insert(name.clone(), previous.clone());
                }
            } else {
                state.env.remove(name);
            }
        }

        for param in &self.params {
            if bound.contains_param(param) {
                self.sensitive_params.insert(param.clone());
            }
        }

        if tainted && !bound.is_bounded() {
            let source = origins
                .iter()
                .next()
                .cloned()
                .unwrap_or_else(|| String::from("request input"));
            self.report(
                ResourceClass::InputCardinality,
                span,
                "E0046:input-cardinality",
                "Input-controlled loop cardinality from {source} reaches a loop without a bound; worst-case runtime {cost}. Reject or cap the input before iterating.",
                &[
                    ("source", source),
                    ("cost", loop_cost.to_string()),
                ],
            );
        }

        if loop_cost.degree() >= self.settings.runtime_degree()
            && loop_cost.degree() > 0
            && !loop_cost.is_bounded()
            && !bound.is_unknown()
        {
            self.report(
                ResourceClass::RuntimeGrowth,
                span,
                "E0044:runtime-growth",
                "Runtime grows as {cost} with {bound}. Nested loops or repeated linear work make this input-size dependent.",
                &[
                    ("cost", loop_cost.to_string()),
                    ("bound", bound.to_string()),
                ],
            );
        }

        let retained_growth = body_effect.retained_delta.clone().mul(iterations.clone());
        let large_retained = self.large_retained_elements(&body_effect);
        let allocation_growth = loop_alloc.degree() >= self.settings.heap_degree()
            && loop_alloc.degree() > 0
            && !loop_alloc.is_bounded();
        let retains = !body_effect.growth.is_empty() || body_effect.retains;
        let significant_retention = large_retained || body_effect.retains;
        if retains
            && (allocation_growth || (significant_retention && !retained_growth.is_bounded()))
            && !bound.is_unknown()
            && !bound.is_bounded()
        {
            self.report(
                ResourceClass::LiveHeap,
                span,
                "E0045:live-heap-growth",
                "Live heap grows with {bound}: retained {retained}, cumulative allocation {alloc}, peak {peak}. Stream or release values instead of accumulating them.",
                &[
                    ("bound", bound.to_string()),
                    ("retained", loop_retained.to_string()),
                    ("alloc", loop_alloc.to_string()),
                    ("peak", loop_peak.to_string()),
                ],
            );
        }

        let mut effect = Effect {
            cost: loop_cost,
            alloc: loop_alloc,
            retained_delta: retained_growth,
            peak: loop_peak,
            tainted,
            origins,
            growth: HashMap::new(),
            retains: body_effect.retains || !body_effect.growth.is_empty(),
        };

        if copy_growth {
            effect.cost = effect.cost.add(CostExpr::quadratic(iterations));
        }

        effect
    }

    fn large_retained_elements(&self, effect: &Effect) -> bool {
        effect
            .growth
            .values()
            .any(|growth| match growth.bytes_per_iteration.as_const() {
                Some(bytes) => bytes >= self.settings.retained_element_bytes,
                None => true,
            })
    }
}

fn else_state_from(state: &State, guards: &[Guard]) -> State {
    state.clone().with_caps(guards)
}

fn sum_bytes(env: &HashMap<String, ValueState>) -> SizeExpr {
    env.values().fold(SizeExpr::Const(0), |total, value| {
        total.add(value.bytes.clone())
    })
}

fn statements_always_exit(statements: &[Statement<'_>]) -> bool {
    match statements.last() {
        Some(Statement::Return(_)) => true,
        Some(Statement::Expression(expression)) => expression_always_exits(expression.expression),
        Some(Statement::Block(block)) => statements_always_exit(block.statements.as_slice()),
        _ => false,
    }
}

fn expression_always_exits(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Throw(_) => true,
        Expression::Call(Call::Function(call)) => {
            function_name(call.function).is_some_and(|name| matches!(name.as_str(), "exit" | "die"))
        }
        _ => false,
    }
}

fn direct_variable_name<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression.unparenthesized() {
        Expression::Variable(Variable::Direct(variable)) => std::str::from_utf8(variable.name).ok(),
        _ => None,
    }
}

fn function_name(expression: &Expression<'_>) -> Option<String> {
    match expression.unparenthesized() {
        Expression::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.last_segment()).to_ascii_lowercase())
        }
        Expression::ConstantAccess(access) => {
            Some(String::from_utf8_lossy(access.name.last_segment()).to_ascii_lowercase())
        }
        _ => None,
    }
}

fn method_name(selector: &ClassLikeMemberSelector<'_>) -> Option<String> {
    match selector {
        ClassLikeMemberSelector::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.value).to_ascii_lowercase())
        }
        _ => None,
    }
}

fn property_name(selector: &ClassLikeMemberSelector<'_>) -> Option<String> {
    match selector {
        ClassLikeMemberSelector::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.value).to_ascii_lowercase())
        }
        _ => None,
    }
}

fn is_request_object(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Variable(Variable::Direct(variable)) => String::from_utf8_lossy(variable.name)
            .to_ascii_lowercase()
            .contains("request"),
        Expression::Access(Access::Property(access)) => {
            property_name(&access.property).is_some_and(|name| name.contains("request"))
        }
        Expression::Access(Access::NullSafeProperty(access)) => {
            property_name(&access.property).is_some_and(|name| name.contains("request"))
        }
        _ => false,
    }
}

fn literal_u64(expression: &Expression<'_>) -> Option<u64> {
    match expression.unparenthesized() {
        Expression::Literal(Literal::Integer(integer)) => integer.value,
        Expression::Literal(Literal::True(_)) => Some(1),
        Expression::Literal(Literal::False(_)) => Some(0),
        Expression::Parenthesized(parenthesized) => literal_u64(parenthesized.expression),
        _ => None,
    }
}

fn literal_string(expression: &Expression<'_>) -> Option<String> {
    match expression.unparenthesized() {
        Expression::Literal(Literal::String(string)) => string
            .value
            .map(|value| String::from_utf8_lossy(value).to_string()),
        Expression::Parenthesized(parenthesized) => literal_string(parenthesized.expression),
        _ => None,
    }
}

impl<'a> Analyzer<'a> {
    fn lookup(&self, name: &str, state: &State) -> ValueState {
        if let Some(source) = self.source_value(name) {
            return source;
        }
        let mut value = state.env.get(name).cloned().unwrap_or_default();
        if let Some(cap) = state.caps.get(name) {
            value.count = value.count.clone().min(cap.clone());
            if let Some(element_bytes) = value.element_bytes.as_const() {
                value.bytes = value
                    .bytes
                    .clone()
                    .min(cap.clone().mul_const(element_bytes));
            }
        }
        value
    }

    fn source_value(&self, name: &str) -> Option<ValueState> {
        let name = name.strip_prefix('$').unwrap_or(name);
        let is_http = matches!(
            name,
            "_GET" | "_POST" | "_REQUEST" | "_COOKIE" | "_FILES" | "_SERVER"
        );
        let is_cli = self.settings.include_cli_input && name == "argv";
        if !is_http && !is_cli {
            return None;
        }
        let mut origins = BTreeSet::new();
        origins.insert(format!("${name}"));
        Some(ValueState {
            count: SizeExpr::Unknown,
            bytes: SizeExpr::Unknown,
            element_bytes: SizeExpr::Unknown,
            alloc: CostExpr::Constant,
            peak: SizeExpr::Unknown,
            tainted: true,
            taint_origins: origins,
            stream: false,
            scalar: false,
            growth: None,
        })
    }

    fn size_of_expr(&self, expression: &Expression<'_>, state: &State) -> SizeExpr {
        if self.size_depth.get() >= 32 {
            return SizeExpr::Unknown;
        }
        self.size_depth.set(self.size_depth.get() + 1);
        let result = self.size_of_expr_inner(expression, state);
        self.size_depth.set(self.size_depth.get() - 1);
        result
    }

    fn size_of_expr_inner(&self, expression: &Expression<'_>, state: &State) -> SizeExpr {
        match expression.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                let name = String::from_utf8_lossy(variable.name);
                self.lookup(&name, state).count
            }
            Expression::Literal(Literal::Integer(integer)) => integer
                .value
                .map(SizeExpr::Const)
                .unwrap_or(SizeExpr::Unknown),
            Expression::Literal(Literal::String(string)) => string
                .value
                .map(|value| SizeExpr::Const(value.len() as u64))
                .unwrap_or(SizeExpr::Unknown),
            Expression::Literal(_) => SizeExpr::Const(1),
            Expression::Array(array) => array_size(&array.elements, self, state),
            Expression::LegacyArray(array) => array_size(&array.elements, self, state),
            Expression::ArrayAccess(access) => {
                let base = self.size_of_expr(access.array, state);
                if base.is_unknown() {
                    SizeExpr::Unknown
                } else {
                    base.min(SizeExpr::Const(1))
                }
            }
            Expression::ArrayAppend(append) => self
                .size_of_expr(append.array, state)
                .add(SizeExpr::Const(1)),
            Expression::Binary(binary) => match &binary.operator {
                BinaryOperator::Addition(_) => self
                    .size_of_expr(binary.lhs, state)
                    .add(self.size_of_expr(binary.rhs, state)),
                BinaryOperator::Subtraction(_) => self.size_of_expr(binary.lhs, state),
                BinaryOperator::Multiplication(_) => self
                    .size_of_expr(binary.lhs, state)
                    .mul(self.size_of_expr(binary.rhs, state)),
                BinaryOperator::StringConcat(_) => self
                    .size_of_expr(binary.lhs, state)
                    .add(self.size_of_expr(binary.rhs, state)),
                BinaryOperator::NullCoalesce(_) => self
                    .size_of_expr(binary.lhs, state)
                    .max(self.size_of_expr(binary.rhs, state)),
                BinaryOperator::LessThan(_)
                | BinaryOperator::LessThanOrEqual(_)
                | BinaryOperator::GreaterThan(_)
                | BinaryOperator::GreaterThanOrEqual(_)
                | BinaryOperator::Equal(_)
                | BinaryOperator::NotEqual(_)
                | BinaryOperator::Identical(_)
                | BinaryOperator::NotIdentical(_)
                | BinaryOperator::AngledNotEqual(_)
                | BinaryOperator::Spaceship(_)
                | BinaryOperator::And(_)
                | BinaryOperator::Or(_)
                | BinaryOperator::LowAnd(_)
                | BinaryOperator::LowOr(_)
                | BinaryOperator::LowXor(_) => SizeExpr::Const(1),
                _ => SizeExpr::Unknown,
            },
            Expression::UnaryPrefix(unary) => self.size_of_expr(unary.operand, state),
            Expression::UnaryPostfix(unary) => self.size_of_expr(unary.operand, state),
            Expression::Parenthesized(parenthesized) => {
                self.size_of_expr(parenthesized.expression, state)
            }
            Expression::Conditional(conditional) => {
                let then = conditional
                    .then
                    .as_ref()
                    .map(|value| self.size_of_expr(value, state))
                    .unwrap_or_else(|| self.size_of_expr(conditional.condition, state));
                then.max(self.size_of_expr(conditional.r#else, state))
            }
            Expression::Assignment(assignment) => self.size_of_expr(assignment.rhs, state),
            Expression::Call(call) => self.size_of_call(call, state),
            Expression::CompositeString(string) => {
                string
                    .parts()
                    .iter()
                    .fold(SizeExpr::Const(0), |total, part| match part {
                        StringPart::Literal(literal) => total.add(SizeExpr::Const(
                            literal.value.map_or(0, |value| value.len() as u64),
                        )),
                        StringPart::Expression(expression) => {
                            total.add(self.size_of_expr(expression, state))
                        }
                        StringPart::BracedExpression(braced) => {
                            total.add(self.size_of_expr(braced.expression, state))
                        }
                    })
            }
            _ => SizeExpr::Unknown,
        }
    }

    fn size_of_call(&self, call: &Call<'_>, state: &State) -> SizeExpr {
        match call {
            Call::Function(function_call) => {
                let Some(name) = function_name(function_call.function) else {
                    return SizeExpr::Unknown;
                };
                let arguments = function_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                let sizes = arguments
                    .iter()
                    .map(|argument| self.size_of_expr(argument.value(), state))
                    .collect::<Vec<_>>();
                match name.as_str() {
                    "count" | "sizeof" => sizes.first().cloned().unwrap_or(SizeExpr::Unknown),
                    "strlen" | "mb_strlen" => sizes.first().cloned().unwrap_or(SizeExpr::Unknown),
                    "array_merge" => sizes
                        .iter()
                        .fold(SizeExpr::Const(0), |total, size| total.add(size.clone())),
                    "array_slice" => {
                        let base = sizes.first().cloned().unwrap_or(SizeExpr::Unknown);
                        match arguments
                            .get(2)
                            .and_then(|argument| literal_u64(argument.value()))
                        {
                            Some(limit) => base.min(SizeExpr::Const(limit)),
                            None => base,
                        }
                    }
                    "array_fill" => sizes.get(1).cloned().unwrap_or(SizeExpr::Unknown),
                    "range" => sizes
                        .get(1)
                        .cloned()
                        .unwrap_or(SizeExpr::Unknown)
                        .add(SizeExpr::Const(1)),
                    "str_repeat" => {
                        let base = sizes.first().cloned().unwrap_or(SizeExpr::Unknown);
                        match arguments
                            .get(1)
                            .and_then(|argument| literal_u64(argument.value()))
                        {
                            Some(count) => base.mul_const(count),
                            None => base.mul(sizes.get(1).cloned().unwrap_or(SizeExpr::Unknown)),
                        }
                    }
                    "array_values" | "array_reverse" | "array_filter" | "array_unique"
                    | "array_keys" | "array_flip" | "array_chunk" | "array_combine"
                    | "array_map" => sizes.first().cloned().unwrap_or(SizeExpr::Unknown),
                    "min" => sizes
                        .into_iter()
                        .reduce(SizeExpr::min)
                        .unwrap_or(SizeExpr::Unknown),
                    "max" => sizes
                        .into_iter()
                        .reduce(SizeExpr::max)
                        .unwrap_or(SizeExpr::Unknown),
                    _ => self
                        .summaries
                        .get(&name)
                        .map(|summary| {
                            let replacements = self.summary_replacements(
                                &summary.params,
                                &arguments,
                                state,
                                false,
                            );
                            summary.return_count.substitute(&replacements)
                        })
                        .unwrap_or(SizeExpr::Unknown),
                }
            }
            Call::Method(method_call) => {
                let Some(name) = method_name(&method_call.method) else {
                    return SizeExpr::Unknown;
                };
                let arguments = method_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                self.summary_for_method(None, &name)
                    .map(|summary| {
                        let replacements =
                            self.summary_replacements(&summary.params, &arguments, state, false);
                        summary.return_count.substitute(&replacements)
                    })
                    .unwrap_or(SizeExpr::Unknown)
            }
            Call::NullSafeMethod(method_call) => {
                let Some(name) = method_name(&method_call.method) else {
                    return SizeExpr::Unknown;
                };
                let arguments = method_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                self.summary_for_method(None, &name)
                    .map(|summary| {
                        let replacements =
                            self.summary_replacements(&summary.params, &arguments, state, false);
                        summary.return_count.substitute(&replacements)
                    })
                    .unwrap_or(SizeExpr::Unknown)
            }
            Call::StaticMethod(static_call) => {
                let Some(name) = method_name(&static_call.method) else {
                    return SizeExpr::Unknown;
                };
                let class = expression_name(static_call.class);
                let arguments = static_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                self.summary_for_method(class.as_deref(), &name)
                    .map(|summary| {
                        let replacements =
                            self.summary_replacements(&summary.params, &arguments, state, false);
                        summary.return_count.substitute(&replacements)
                    })
                    .unwrap_or(SizeExpr::Unknown)
            }
        }
    }

    fn taint_of_expr(&self, expression: &Expression<'_>, state: &State) -> BTreeSet<String> {
        if self.taint_depth.get() >= 32 {
            return BTreeSet::new();
        }
        self.taint_depth.set(self.taint_depth.get() + 1);
        let result = self.taint_of_expr_inner(expression, state);
        self.taint_depth.set(self.taint_depth.get() - 1);
        result
    }

    fn taint_of_expr_inner(&self, expression: &Expression<'_>, state: &State) -> BTreeSet<String> {
        match expression.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                let name = String::from_utf8_lossy(variable.name);
                let value = self.lookup(&name, state);
                if value.tainted {
                    value.taint_origins
                } else {
                    BTreeSet::new()
                }
            }
            Expression::ArrayAccess(access) => self.taint_of_expr(access.array, state),
            Expression::ArrayAppend(append) => self.taint_of_expr(append.array, state),
            Expression::Array(array) => array_origins(&array.elements, self, state),
            Expression::LegacyArray(array) => array_origins(&array.elements, self, state),
            Expression::Assignment(assignment) => self.taint_of_expr(assignment.rhs, state),
            Expression::Binary(binary) => {
                let mut origins = self.taint_of_expr(binary.lhs, state);
                origins.extend(self.taint_of_expr(binary.rhs, state));
                origins
            }
            Expression::UnaryPrefix(unary) => self.taint_of_expr(unary.operand, state),
            Expression::UnaryPostfix(unary) => self.taint_of_expr(unary.operand, state),
            Expression::Parenthesized(parenthesized) => {
                self.taint_of_expr(parenthesized.expression, state)
            }
            Expression::Conditional(conditional) => {
                let mut origins = self.taint_of_expr(conditional.condition, state);
                if let Some(then) = conditional.then {
                    origins.extend(self.taint_of_expr(then, state));
                }
                origins.extend(self.taint_of_expr(conditional.r#else, state));
                origins
            }
            Expression::Call(call) => self.taint_of_call(call, state),
            Expression::CompositeString(string) => {
                let mut origins = BTreeSet::new();
                for part in string.parts().iter() {
                    match part {
                        StringPart::Literal(_) => {}
                        StringPart::Expression(expression) => {
                            origins.extend(self.taint_of_expr(expression, state));
                        }
                        StringPart::BracedExpression(braced) => {
                            origins.extend(self.taint_of_expr(braced.expression, state));
                        }
                    }
                }
                origins
            }
            _ => BTreeSet::new(),
        }
    }

    fn taint_of_call(&self, call: &Call<'_>, state: &State) -> BTreeSet<String> {
        match call {
            Call::Function(function_call) => {
                let Some(name) = function_name(function_call.function) else {
                    return BTreeSet::new();
                };
                let arguments = function_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                if is_source_function(&name) {
                    let mut origins = BTreeSet::new();
                    origins.insert(format!("{name}()"));
                    return origins;
                }
                if name == "file_get_contents" {
                    if let Some(argument) = arguments.first() {
                        if literal_string(argument.value())
                            .is_some_and(|path| path.contains("php://input"))
                        {
                            let mut origins = BTreeSet::new();
                            origins.insert(String::from("php://input"));
                            return origins;
                        }
                    }
                }
                let mut origins = BTreeSet::new();
                for argument in &arguments {
                    origins.extend(self.taint_of_expr(argument.value(), state));
                }
                if let Some(summary) = self.summaries.get(&name) {
                    for index in &summary.return_tainted_from {
                        if let Some(argument) = arguments.get(*index) {
                            origins.extend(self.taint_of_expr(argument.value(), state));
                        }
                    }
                }
                origins
            }
            Call::Method(method_call) => {
                let Some(name) = method_name(&method_call.method) else {
                    return BTreeSet::new();
                };
                let mut origins = BTreeSet::new();
                for argument in method_call.argument_list.arguments.iter() {
                    origins.extend(self.taint_of_expr(argument.value(), state));
                }
                if is_request_method(&name) && is_request_object(method_call.object) {
                    let object = self.taint_of_expr(method_call.object, state);
                    if object.is_empty() {
                        origins.insert(format!("request->{name}()"));
                    } else {
                        origins.extend(object);
                    }
                }
                if let Some(summary) = self.summary_for_method(None, &name) {
                    let arguments = method_call
                        .argument_list
                        .arguments
                        .iter()
                        .collect::<Vec<_>>();
                    for index in &summary.return_tainted_from {
                        if let Some(argument) = arguments.get(*index) {
                            origins.extend(self.taint_of_expr(argument.value(), state));
                        }
                    }
                }
                origins
            }
            Call::NullSafeMethod(method_call) => {
                let Some(name) = method_name(&method_call.method) else {
                    return BTreeSet::new();
                };
                let mut origins = BTreeSet::new();
                for argument in method_call.argument_list.arguments.iter() {
                    origins.extend(self.taint_of_expr(argument.value(), state));
                }
                if is_request_method(&name) {
                    origins.insert(format!("request->{name}()"));
                }
                origins
            }
            Call::StaticMethod(static_call) => {
                let mut origins = BTreeSet::new();
                for argument in static_call.argument_list.arguments.iter() {
                    origins.extend(self.taint_of_expr(argument.value(), state));
                }
                origins
            }
        }
    }

    fn element_state(&self, expression: &Expression<'_>, state: &State) -> ValueState {
        let origins = self.taint_of_expr(expression, state);
        let tainted = !origins.is_empty();
        let element_bytes = match expression.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                let name = String::from_utf8_lossy(variable.name);
                self.lookup(&name, state).element_bytes
            }
            Expression::Array(array) => array_element_bytes(&array.elements, self, state),
            Expression::LegacyArray(array) => array_element_bytes(&array.elements, self, state),
            Expression::Call(Call::Function(function_call)) => {
                function_name(function_call.function)
                    .and_then(|name| self.summaries.get(&name))
                    .map(|summary| summary.return_bytes.clone())
                    .unwrap_or(SizeExpr::Unknown)
            }
            _ => SizeExpr::Unknown,
        };
        ValueState {
            count: SizeExpr::Const(1),
            bytes: element_bytes.clone(),
            element_bytes,
            alloc: CostExpr::Constant,
            peak: SizeExpr::Const(0),
            tainted,
            taint_origins: origins,
            stream: false,
            scalar: false,
            growth: None,
        }
    }

    fn loop_bound_from_condition(&self, condition: &Expression<'_>, state: &State) -> SizeExpr {
        let Expression::Binary(binary) = condition.unparenthesized() else {
            return SizeExpr::Unknown;
        };
        let left_is_counter = direct_variable_name(binary.lhs).is_some();
        if !left_is_counter {
            return SizeExpr::Unknown;
        }
        match &binary.operator {
            BinaryOperator::LessThan(_) => self.size_of_expr(binary.rhs, state),
            BinaryOperator::LessThanOrEqual(_) => {
                self.size_of_expr(binary.rhs, state).add(SizeExpr::Const(1))
            }
            _ => SizeExpr::Unknown,
        }
    }

    fn guards_from_condition(
        &self,
        condition: &Expression<'_>,
        truth: bool,
        state: &State,
    ) -> Vec<Guard> {
        match condition.unparenthesized() {
            Expression::Parenthesized(parenthesized) => {
                self.guards_from_condition(parenthesized.expression, truth, state)
            }
            Expression::UnaryPrefix(unary) if unary.operator.is_not() => {
                self.guards_from_condition(unary.operand, !truth, state)
            }
            Expression::Binary(binary) => match &binary.operator {
                BinaryOperator::And(_) | BinaryOperator::LowAnd(_) if truth => {
                    let mut guards = self.guards_from_condition(binary.lhs, true, state);
                    guards.extend(self.guards_from_condition(binary.rhs, true, state));
                    guards
                }
                BinaryOperator::Or(_) | BinaryOperator::LowOr(_) if !truth => {
                    let mut guards = self.guards_from_condition(binary.lhs, false, state);
                    guards.extend(self.guards_from_condition(binary.rhs, false, state));
                    guards
                }
                BinaryOperator::LessThan(_)
                | BinaryOperator::LessThanOrEqual(_)
                | BinaryOperator::GreaterThan(_)
                | BinaryOperator::GreaterThanOrEqual(_) => {
                    self.comparison_guard(binary, truth, state)
                }
                _ => Vec::new(),
            },
            _ => Vec::new(),
        }
    }

    fn comparison_guard(&self, binary: &Binary<'_>, truth: bool, state: &State) -> Vec<Guard> {
        let left = self.size_variable(binary.lhs, state);
        let right = literal_u64(binary.rhs);
        if let (Some((name, _)), Some(value)) = (left, right) {
            return self.cap_for_operator(&binary.operator, truth, &name, value);
        }

        let left_literal = literal_u64(binary.lhs);
        let right_var = self.size_variable(binary.rhs, state);
        if let (Some(value), Some((name, _))) = (left_literal, right_var) {
            let inverted = invert_comparison(&binary.operator);
            if let Some(inverted) = inverted {
                return self.cap_for_operator(&inverted, truth, &name, value);
            }
        }
        Vec::new()
    }

    fn cap_for_operator(
        &self,
        operator: &BinaryOperator<'_>,
        truth: bool,
        name: &str,
        value: u64,
    ) -> Vec<Guard> {
        let cap = match (operator, truth) {
            (BinaryOperator::LessThanOrEqual(_), true) => Some(value),
            (BinaryOperator::LessThan(_), true) => Some(value.saturating_sub(1)),
            (BinaryOperator::GreaterThan(_), false) => Some(value),
            (BinaryOperator::GreaterThanOrEqual(_), false) => Some(value.saturating_sub(1)),
            _ => None,
        };
        cap.map(|cap| {
            vec![Guard {
                name: name.to_string(),
                cap: SizeExpr::Const(cap),
            }]
        })
        .unwrap_or_default()
    }

    fn size_variable(
        &self,
        expression: &Expression<'_>,
        state: &State,
    ) -> Option<(String, SizeExpr)> {
        if let Some(name) = direct_variable_name(expression) {
            return Some((name.to_string(), self.lookup(name, state).count));
        }
        if let Expression::Call(Call::Function(call)) = expression.unparenthesized() {
            let name = function_name(call.function)?;
            if matches!(name.as_str(), "count" | "sizeof" | "strlen" | "mb_strlen") {
                let argument = call.argument_list.arguments.iter().next()?;
                let variable = direct_variable_name(argument.value())?;
                return Some((variable.to_string(), self.size_of_expr(expression, state)));
            }
        }
        None
    }

    fn summary_for_method(&self, class: Option<&str>, method: &str) -> Option<&ResourceSummary> {
        if let Some(class) = class {
            if let Some(summary) = self.summaries.get(&format!("{class}::{method}")) {
                return Some(summary);
            }
        }
        self.summaries.get(method)
    }

    fn summary_replacements(
        &self,
        params: &[String],
        arguments: &[&Argument<'_>],
        state: &State,
        bytes: bool,
    ) -> BTreeMap<String, SizeExpr> {
        let mut replacements = BTreeMap::new();
        for (index, param) in params.iter().enumerate() {
            let size = arguments
                .get(index)
                .map(|argument| {
                    let value = self.size_of_expr(argument.value(), state);
                    if bytes {
                        self.bytes_of_expr(argument.value(), state)
                    } else {
                        value
                    }
                })
                .unwrap_or(SizeExpr::Unknown);
            replacements.insert(param.clone(), size);
        }
        replacements
    }

    fn bytes_of_expr(&self, expression: &Expression<'_>, state: &State) -> SizeExpr {
        match expression.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                let name = String::from_utf8_lossy(variable.name);
                self.lookup(&name, state).bytes
            }
            Expression::Array(array) => array_elements_bytes(&array.elements, self, state),
            Expression::LegacyArray(array) => array_elements_bytes(&array.elements, self, state),
            Expression::Literal(Literal::String(string)) => string
                .value
                .map(|value| SizeExpr::Const(value.len() as u64))
                .unwrap_or(SizeExpr::Unknown),
            Expression::Literal(_) => SizeExpr::Const(8),
            _ => self.size_of_expr(expression, state),
        }
    }

    fn report(
        &mut self,
        class: ResourceClass,
        span: Span,
        id: &str,
        template: &str,
        args: &[(&str, String)],
    ) {
        if self.summary_mode || !self.classes.contains(&class) {
            return;
        }
        if class == ResourceClass::InputCardinality && !self.settings.report_input_cardinality {
            return;
        }
        if self.findings.len() >= self.settings.max_findings_per_function {
            return;
        }
        let mut message = Message::new(id, template);
        for (key, value) in args {
            message = message.arg(*key, value.clone());
        }
        self.findings.push(ResourceFinding {
            class,
            message,
            span,
        });
    }
}

fn array_size(
    elements: &TokenSeparatedSequence<'_, ArrayElement<'_>>,
    analyzer: &Analyzer<'_>,
    state: &State,
) -> SizeExpr {
    let mut count = SizeExpr::Const(0);
    for element in elements.iter() {
        match element {
            ArrayElement::KeyValue(element) => {
                count = count.add(analyzer.size_of_expr(element.value, state));
            }
            ArrayElement::Value(element) => {
                count = count.add(analyzer.size_of_expr(element.value, state));
            }
            ArrayElement::Variadic(_) => return SizeExpr::Unknown,
            ArrayElement::Missing(_) => {}
        }
    }
    count
}

fn array_origins(
    elements: &TokenSeparatedSequence<'_, ArrayElement<'_>>,
    analyzer: &Analyzer<'_>,
    state: &State,
) -> BTreeSet<String> {
    let mut origins = BTreeSet::new();
    for element in elements.iter() {
        if let Some(value) = element.get_value() {
            origins.extend(analyzer.taint_of_expr(value, state));
        }
    }
    origins
}

fn array_element_bytes(
    elements: &TokenSeparatedSequence<'_, ArrayElement<'_>>,
    analyzer: &Analyzer<'_>,
    state: &State,
) -> SizeExpr {
    elements
        .iter()
        .filter_map(ArrayElement::get_value)
        .fold(SizeExpr::Const(0), |max, value| {
            max.max(analyzer.bytes_of_expr(value, state))
        })
}

fn array_elements_bytes(
    elements: &TokenSeparatedSequence<'_, ArrayElement<'_>>,
    analyzer: &Analyzer<'_>,
    state: &State,
) -> SizeExpr {
    elements
        .iter()
        .filter_map(ArrayElement::get_value)
        .fold(SizeExpr::Const(0), |total, value| {
            total.add(analyzer.bytes_of_expr(value, state))
        })
}

fn expression_name(expression: &Expression<'_>) -> Option<String> {
    match expression.unparenthesized() {
        Expression::Identifier(identifier) => {
            Some(String::from_utf8_lossy(identifier.last_segment()).to_string())
        }
        Expression::Self_(_) => Some(String::from("self")),
        Expression::Static(_) => Some(String::from("static")),
        Expression::Parent(_) => Some(String::from("parent")),
        _ => None,
    }
}

fn invert_comparison<'arena>(operator: &BinaryOperator<'arena>) -> Option<BinaryOperator<'arena>> {
    match operator {
        BinaryOperator::LessThan(span) => Some(BinaryOperator::GreaterThan(*span)),
        BinaryOperator::LessThanOrEqual(span) => Some(BinaryOperator::GreaterThanOrEqual(*span)),
        BinaryOperator::GreaterThan(span) => Some(BinaryOperator::LessThan(*span)),
        BinaryOperator::GreaterThanOrEqual(span) => Some(BinaryOperator::LessThanOrEqual(*span)),
        _ => None,
    }
}

fn is_source_function(name: &str) -> bool {
    matches!(
        name,
        "filter_input" | "getenv" | "readline" | "apache_request_headers" | "getallheaders"
    )
}

fn is_request_method(name: &str) -> bool {
    matches!(
        name,
        "input"
            | "query"
            | "post"
            | "get"
            | "all"
            | "json"
            | "file"
            | "cookie"
            | "header"
            | "param"
    )
}

impl<'a> Analyzer<'a> {
    fn eval_expr(&mut self, expression: &Expression<'_>, state: &mut State) -> Eval {
        if self.expr_depth >= 32 {
            return Eval::pure(ValueState::default());
        }
        self.expr_depth += 1;
        let result = self.eval_expr_inner(expression, state);
        self.expr_depth -= 1;
        result
    }

    fn eval_expr_inner(&mut self, expression: &Expression<'_>, state: &mut State) -> Eval {
        match expression.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                let name = String::from_utf8_lossy(variable.name);
                Eval::pure(self.lookup(&name, state))
            }
            Expression::Variable(_) => Eval::pure(ValueState::default()),
            Expression::Literal(Literal::String(string)) => {
                let bytes = string
                    .value
                    .map(|value| SizeExpr::Const(value.len() as u64))
                    .unwrap_or(SizeExpr::Unknown);
                Eval::pure(ValueState::string(bytes, false))
            }
            Expression::Literal(Literal::Integer(integer)) => {
                let mut value = ValueState::scalar();
                value.count = integer
                    .value
                    .map(SizeExpr::Const)
                    .unwrap_or(SizeExpr::Unknown);
                Eval::pure(value)
            }
            Expression::Literal(_) => Eval::pure(ValueState::scalar()),
            Expression::Array(array) => self.eval_array(&array.elements, state),
            Expression::LegacyArray(array) => self.eval_array(&array.elements, state),
            Expression::ArrayAccess(access) => {
                let array = self.eval_expr(access.array, state);
                let index = self.eval_expr(access.index, state);
                let element_bytes = array.value.element_bytes.clone();
                let input_cardinality = array.value.tainted && array.value.count.is_unknown();
                let mut value = ValueState {
                    count: if input_cardinality {
                        array.value.count.clone()
                    } else {
                        SizeExpr::Const(1)
                    },
                    bytes: if input_cardinality {
                        array.value.bytes.clone()
                    } else {
                        element_bytes.clone()
                    },
                    element_bytes,
                    alloc: CostExpr::Constant,
                    peak: SizeExpr::Const(0),
                    tainted: array.value.tainted,
                    taint_origins: array.value.taint_origins.clone(),
                    stream: false,
                    scalar: !input_cardinality,
                    growth: None,
                };
                value.tainted |= index.value.tainted;
                value
                    .taint_origins
                    .extend(index.value.taint_origins.clone());
                Eval {
                    value,
                    effect: array.effect.add(index.effect),
                }
            }
            Expression::ArrayAppend(append) => {
                let array = self.eval_expr(append.array, state);
                let mut value = array.value.clone();
                value.count = value.count.add(SizeExpr::Const(1));
                Eval {
                    value,
                    effect: array.effect,
                }
            }
            Expression::Binary(binary) => self.eval_binary(binary, state),
            Expression::UnaryPrefix(unary) => self.eval_unary_prefix(unary, state),
            Expression::UnaryPostfix(unary) => self.eval_unary_postfix(unary, state),
            Expression::Parenthesized(parenthesized) => {
                self.eval_expr(parenthesized.expression, state)
            }
            Expression::Assignment(assignment) => self.eval_assignment(assignment, state),
            Expression::Conditional(conditional) => self.eval_conditional(conditional, state),
            Expression::Call(call) => self.eval_call(call, state),
            Expression::Access(access) => self.eval_access(access, state),
            Expression::Instantiation(instantiation) => {
                let mut effect = Effect::default();
                if let Some(arguments) = &instantiation.argument_list {
                    for argument in arguments.arguments.iter() {
                        effect = effect.add(self.eval_expr(argument.value(), state).effect);
                    }
                }
                let mut value = ValueState::default();
                value.bytes = SizeExpr::Unknown;
                value.peak = SizeExpr::Unknown;
                value.tainted = effect.tainted;
                value.taint_origins = effect.origins.clone();
                Eval { value, effect }
            }
            Expression::Clone(clone) => self.eval_expr(clone.object, state),
            Expression::CompositeString(string) => {
                let mut effect = Effect::default();
                let mut bytes = SizeExpr::Const(0);
                let mut origins = BTreeSet::new();
                for part in string.parts().iter() {
                    match part {
                        StringPart::Literal(literal) => {
                            bytes = bytes.add(SizeExpr::Const(
                                literal.value.map_or(0, |value| value.len() as u64),
                            ));
                        }
                        StringPart::Expression(expression) => {
                            let evaluated = self.eval_expr(expression, state);
                            bytes = bytes.add(evaluated.value.bytes.clone());
                            origins.extend(evaluated.value.taint_origins);
                            effect = effect.add(evaluated.effect);
                        }
                        StringPart::BracedExpression(braced) => {
                            let evaluated = self.eval_expr(braced.expression, state);
                            bytes = bytes.add(evaluated.value.bytes.clone());
                            origins.extend(evaluated.value.taint_origins);
                            effect = effect.add(evaluated.effect);
                        }
                    }
                }
                let mut value = ValueState::string(bytes, false);
                value.taint_origins = origins;
                value.tainted = !value.taint_origins.is_empty();
                Eval { value, effect }
            }
            Expression::Construct(construct) => self.eval_construct(construct, state),
            Expression::Throw(throw) => {
                let value = self.eval_expr(throw.exception, state);
                Eval {
                    value: ValueState::default(),
                    effect: value.effect,
                }
            }
            Expression::Yield(yield_expression) => {
                let mut effect = Effect::default();
                let yielded = match yield_expression {
                    Yield::Value(value) => value.value,
                    Yield::Pair(pair) => Some(pair.value),
                    Yield::From(from) => Some(from.iterator),
                };
                if let Some(value) = yielded {
                    effect = effect.add(self.eval_expr(value, state).effect);
                }
                Eval {
                    value: ValueState::default(),
                    effect,
                }
            }
            Expression::Match(r#match) => {
                let condition = self.eval_expr(r#match.expression, state);
                let mut effect = condition.effect;
                let mut value = ValueState::default();
                for arm in r#match.arms.iter() {
                    let mut arm_effect = Effect::default();
                    if let MatchArm::Expression(arm) = arm {
                        for condition in arm.conditions.iter() {
                            arm_effect = arm_effect.add(self.eval_expr(condition, state).effect);
                        }
                    }
                    let result = self.eval_expr(arm.expression(), state);
                    value = ValueState::merge(&value, &result.value);
                    arm_effect = arm_effect.add(result.effect);
                    effect = effect.branch_merge(arm_effect);
                }
                Eval { value, effect }
            }
            _ => Eval::pure(ValueState::default()),
        }
    }

    fn eval_array(
        &mut self,
        elements: &TokenSeparatedSequence<'_, ArrayElement<'_>>,
        state: &mut State,
    ) -> Eval {
        let mut effect = Effect::default();
        let mut count = SizeExpr::Const(0);
        let mut bytes = SizeExpr::Const(0);
        let mut element_bytes = SizeExpr::Const(0);
        let mut tainted = false;
        let mut origins = BTreeSet::new();
        for element in elements.iter() {
            let Some(value) = element.get_value() else {
                continue;
            };
            let evaluated = self.eval_expr(value, state);
            count = count.add(SizeExpr::Const(1));
            bytes = bytes.add(evaluated.value.bytes.clone());
            element_bytes = element_bytes.max(evaluated.value.bytes.clone());
            tainted |= evaluated.value.tainted;
            origins.extend(evaluated.value.taint_origins);
            effect = effect.add(evaluated.effect);
        }
        let mut value = ValueState::array(count, bytes.clone(), element_bytes, tainted);
        value.taint_origins = origins;
        value.alloc = CostExpr::linear(bytes.clone());
        value.peak = bytes;
        Eval { value, effect }
    }

    fn eval_binary(&mut self, binary: &Binary<'_>, state: &mut State) -> Eval {
        let left = self.eval_expr(binary.lhs, state);
        let right = self.eval_expr(binary.rhs, state);
        let mut effect = left.effect.add(right.effect);
        let mut value = match &binary.operator {
            BinaryOperator::StringConcat(_) => {
                let bytes = left.value.bytes.clone().add(right.value.bytes.clone());
                let mut value = ValueState::string(bytes.clone(), false);
                value.alloc = left
                    .value
                    .alloc
                    .clone()
                    .add(right.value.alloc.clone())
                    .add(CostExpr::linear(bytes));
                value.peak = value.bytes.clone();
                value
            }
            BinaryOperator::Addition(_) => {
                let mut value = ValueState::scalar();
                value.count = left.value.count.clone().add(right.value.count.clone());
                value
            }
            BinaryOperator::Subtraction(_) => {
                let mut value = ValueState::scalar();
                value.count = left.value.count.clone();
                value
            }
            BinaryOperator::Multiplication(_) => {
                let mut value = ValueState::scalar();
                value.count = left.value.count.clone().mul(right.value.count.clone());
                value
            }
            BinaryOperator::Division(_) | BinaryOperator::Modulo(_) => {
                let mut value = ValueState::scalar();
                value.count = left.value.count.clone();
                value
            }
            BinaryOperator::NullCoalesce(_) => ValueState::merge(&left.value, &right.value),
            BinaryOperator::And(_)
            | BinaryOperator::Or(_)
            | BinaryOperator::LowAnd(_)
            | BinaryOperator::LowOr(_)
            | BinaryOperator::LowXor(_)
            | BinaryOperator::Equal(_)
            | BinaryOperator::NotEqual(_)
            | BinaryOperator::Identical(_)
            | BinaryOperator::NotIdentical(_)
            | BinaryOperator::AngledNotEqual(_)
            | BinaryOperator::LessThan(_)
            | BinaryOperator::LessThanOrEqual(_)
            | BinaryOperator::GreaterThan(_)
            | BinaryOperator::GreaterThanOrEqual(_)
            | BinaryOperator::Spaceship(_)
            | BinaryOperator::Instanceof(_) => ValueState::scalar(),
            _ => {
                let mut value = ValueState::scalar();
                value.count = left.value.count.clone();
                value
            }
        };
        value.tainted = left.value.tainted || right.value.tainted;
        value.taint_origins = left
            .value
            .taint_origins
            .union(&right.value.taint_origins)
            .cloned()
            .collect();
        effect.tainted = value.tainted;
        effect.origins = value.taint_origins.clone();
        Eval { value, effect }
    }

    fn eval_unary_prefix(&mut self, unary: &UnaryPrefix<'_>, state: &mut State) -> Eval {
        let operand = self.eval_expr(unary.operand, state);
        if unary.operator.is_cast() {
            let mut value = ValueState::scalar();
            value.count = operand.value.count.clone();
            value.tainted = operand.value.tainted;
            value.taint_origins = operand.value.taint_origins.clone();
            return Eval {
                value,
                effect: operand.effect,
            };
        }
        Eval {
            value: operand.value,
            effect: operand.effect,
        }
    }

    fn eval_unary_postfix(&mut self, unary: &UnaryPostfix<'_>, state: &mut State) -> Eval {
        self.eval_expr(unary.operand, state)
    }

    fn eval_conditional(&mut self, conditional: &Conditional<'_>, state: &mut State) -> Eval {
        let condition = self.eval_expr(conditional.condition, state);
        let then = conditional
            .then
            .as_ref()
            .map(|value| self.eval_expr(value, state))
            .unwrap_or_else(|| condition.clone());
        let r#else = self.eval_expr(conditional.r#else, state);
        let value = ValueState::merge(&then.value, &r#else.value);
        Eval {
            value,
            effect: condition
                .effect
                .add(then.effect.branch_merge(r#else.effect)),
        }
    }

    fn eval_access(&mut self, access: &Access<'_>, state: &mut State) -> Eval {
        let object = match access {
            Access::Property(access) => self.eval_expr(access.object, state),
            Access::NullSafeProperty(access) => self.eval_expr(access.object, state),
            Access::StaticProperty(access) => self.eval_expr(access.class, state),
            Access::ClassConstant(access) => self.eval_expr(access.class, state),
        };
        let mut value = ValueState::default();
        value.tainted = object.value.tainted;
        value.taint_origins = object.value.taint_origins.clone();
        Eval {
            value,
            effect: object.effect,
        }
    }

    fn eval_construct(&mut self, construct: &Construct<'_>, state: &mut State) -> Eval {
        match construct {
            Construct::Isset(isset) => {
                let mut effect = Effect::default();
                for value in isset.values.iter() {
                    effect = effect.add(self.eval_expr(value, state).effect);
                }
                Eval {
                    value: ValueState::scalar(),
                    effect,
                }
            }
            Construct::Empty(empty) => Eval {
                value: ValueState::scalar(),
                effect: self.eval_expr(empty.value, state).effect,
            },
            Construct::Eval(eval) => Eval {
                value: ValueState::scalar(),
                effect: self.eval_expr(eval.value, state).effect,
            },
            Construct::Include(include) => Eval {
                value: ValueState::scalar(),
                effect: self.eval_expr(include.value, state).effect,
            },
            Construct::IncludeOnce(include) => Eval {
                value: ValueState::scalar(),
                effect: self.eval_expr(include.value, state).effect,
            },
            Construct::Require(require) => Eval {
                value: ValueState::scalar(),
                effect: self.eval_expr(require.value, state).effect,
            },
            Construct::RequireOnce(require) => Eval {
                value: ValueState::scalar(),
                effect: self.eval_expr(require.value, state).effect,
            },
            Construct::Print(print) => Eval {
                value: ValueState::scalar(),
                effect: self.eval_expr(print.value, state).effect,
            },
            Construct::Exit(exit) => {
                let mut effect = Effect::default();
                if let Some(arguments) = &exit.arguments {
                    for argument in arguments.arguments.iter() {
                        effect = effect.add(self.eval_expr(argument.value(), state).effect);
                    }
                }
                Eval {
                    value: ValueState::scalar(),
                    effect,
                }
            }
            Construct::Die(die) => {
                let mut effect = Effect::default();
                if let Some(arguments) = &die.arguments {
                    for argument in arguments.arguments.iter() {
                        effect = effect.add(self.eval_expr(argument.value(), state).effect);
                    }
                }
                Eval {
                    value: ValueState::scalar(),
                    effect,
                }
            }
        }
    }
}

impl<'a> Analyzer<'a> {
    fn eval_assignment(&mut self, assignment: &Assignment<'_>, state: &mut State) -> Eval {
        match assignment.lhs.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                let name = String::from_utf8_lossy(variable.name).to_string();
                let mut rhs = self.eval_expr(assignment.rhs, state);
                match &assignment.operator {
                    AssignmentOperator::Assign(_) => {
                        if let Some(growth) = self.detect_growth(assignment.rhs, &name, state) {
                            rhs.effect.retained_delta = growth.bytes_per_iteration.clone();
                            rhs.effect.growth.insert(name.clone(), growth);
                        }
                        state.env.insert(name, rhs.value.clone());
                        rhs
                    }
                    AssignmentOperator::Concat(_) => {
                        let left = self.lookup(&name, state);
                        let bytes = left.bytes.clone().add(rhs.value.bytes.clone());
                        let mut value = ValueState::string(bytes.clone(), false);
                        value.alloc = left
                            .alloc
                            .clone()
                            .add(rhs.value.alloc.clone())
                            .add(CostExpr::linear(bytes.clone()));
                        value.peak = bytes.clone();
                        value.tainted = left.tainted || rhs.value.tainted;
                        value.taint_origins = left
                            .taint_origins
                            .union(&rhs.value.taint_origins)
                            .cloned()
                            .collect();
                        let growth = Growth::copy(rhs.value.count.clone(), rhs.value.bytes.clone());
                        rhs.effect.retained_delta = rhs.value.bytes.clone();
                        rhs.effect.growth.insert(name.clone(), growth);
                        rhs.effect.alloc = rhs.effect.alloc.add(CostExpr::linear(bytes));
                        state.env.insert(name, value.clone());
                        Eval {
                            value,
                            effect: rhs.effect,
                        }
                    }
                    AssignmentOperator::Addition(_) => {
                        let left = self.lookup(&name, state);
                        let mut value = ValueState::scalar();
                        value.count = left.count.clone().add(rhs.value.count.clone());
                        value.tainted = left.tainted || rhs.value.tainted;
                        value.taint_origins = left
                            .taint_origins
                            .union(&rhs.value.taint_origins)
                            .cloned()
                            .collect();
                        state.env.insert(name, value.clone());
                        Eval {
                            value,
                            effect: rhs.effect,
                        }
                    }
                    AssignmentOperator::Multiplication(_) => {
                        let left = self.lookup(&name, state);
                        let mut value = ValueState::scalar();
                        value.count = left.count.clone().mul(rhs.value.count.clone());
                        state.env.insert(name, value.clone());
                        Eval {
                            value,
                            effect: rhs.effect,
                        }
                    }
                    _ => {
                        state.env.insert(name, rhs.value.clone());
                        rhs
                    }
                }
            }
            Expression::ArrayAppend(append) => {
                let Some(name) = direct_variable_name(append.array) else {
                    let array = self.eval_expr(append.array, state);
                    let value = self.eval_expr(assignment.rhs, state);
                    return Eval {
                        value: array.value,
                        effect: array.effect.add(value.effect),
                    };
                };
                let name = name.to_string();
                let base = self.lookup(&name, state);
                let value = self.eval_expr(assignment.rhs, state);
                let mut result = base.clone();
                result.count = result.count.add(SizeExpr::Const(1));
                result.bytes = result.bytes.add(value.value.bytes.clone());
                result.element_bytes = result.element_bytes.max(value.value.bytes.clone());
                result.alloc = result
                    .alloc
                    .add(value.value.alloc.clone())
                    .add(CostExpr::linear(value.value.bytes.clone()));
                result.peak = result.bytes.clone().max(base.peak.clone());
                result.tainted = base.tainted || value.value.tainted;
                result.taint_origins = base
                    .taint_origins
                    .union(&value.value.taint_origins)
                    .cloned()
                    .collect();
                state.env.insert(name.clone(), result.clone());
                let mut effect = value.effect;
                effect.retained_delta = value.value.bytes.clone();
                effect.peak = effect.peak.max(result.bytes.clone());
                effect.alloc = effect
                    .alloc
                    .add(CostExpr::linear(value.value.bytes.clone()));
                effect
                    .growth
                    .insert(name, Growth::append(value.value.bytes.clone()));
                Eval {
                    value: result,
                    effect,
                }
            }
            Expression::ArrayAccess(access) => {
                let Some(name) = direct_variable_name(access.array) else {
                    let array = self.eval_expr(access.array, state);
                    let index = self.eval_expr(access.index, state);
                    let value = self.eval_expr(assignment.rhs, state);
                    return Eval {
                        value: value.value,
                        effect: array.effect.add(index.effect).add(value.effect),
                    };
                };
                let name = name.to_string();
                let base = self.lookup(&name, state);
                let index = self.eval_expr(access.index, state);
                let value = self.eval_expr(assignment.rhs, state);
                let mut result = base.clone();
                result.bytes = result.bytes.add(value.value.bytes.clone());
                result.element_bytes = result.element_bytes.max(value.value.bytes.clone());
                result.peak = result.bytes.clone().max(base.peak.clone());
                result.tainted = base.tainted || value.value.tainted;
                result.taint_origins = base
                    .taint_origins
                    .union(&value.value.taint_origins)
                    .cloned()
                    .collect();
                state.env.insert(name.clone(), result.clone());
                let mut effect = index.effect.add(value.effect);
                effect.retained_delta = value.value.bytes.clone();
                effect
                    .growth
                    .insert(name, Growth::retain(value.value.bytes.clone()));
                Eval {
                    value: result,
                    effect,
                }
            }
            _ => {
                let lhs = self.eval_expr(assignment.lhs, state);
                let rhs = self.eval_expr(assignment.rhs, state);
                Eval {
                    value: rhs.value,
                    effect: lhs.effect.add(rhs.effect),
                }
            }
        }
    }

    fn detect_growth(&self, rhs: &Expression<'_>, name: &str, state: &State) -> Option<Growth> {
        match rhs.unparenthesized() {
            Expression::Call(Call::Function(call)) => {
                let function = function_name(call.function)?;
                if function != "array_merge" {
                    return None;
                }
                let mut arguments = call.argument_list.arguments.iter();
                let first = arguments.next()?;
                if direct_variable_name(first.value()) != Some(name) {
                    return None;
                }
                let mut count = SizeExpr::Const(0);
                let mut bytes = SizeExpr::Const(0);
                for argument in arguments {
                    count = count.add(self.size_of_expr(argument.value(), state));
                    bytes = bytes.add(self.bytes_of_expr(argument.value(), state));
                }
                Some(Growth::copy(count, bytes))
            }
            Expression::Binary(binary) if binary.operator.is_concatenation() => {
                let left = self.taint_of_expr(binary.lhs, state);
                let right = self.taint_of_expr(binary.rhs, state);
                if left.is_empty() && !self.expr_contains_variable(binary.lhs, name) {
                    let _ = right;
                    return None;
                }
                if !self.expr_contains_variable(binary.lhs, name) {
                    return None;
                }
                Some(Growth::copy(
                    self.size_of_expr(binary.rhs, state),
                    self.bytes_of_expr(binary.rhs, state),
                ))
            }
            _ => None,
        }
    }

    fn expr_contains_variable(&self, expression: &Expression<'_>, name: &str) -> bool {
        match expression.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                String::from_utf8_lossy(variable.name) == name
            }
            Expression::Binary(binary) => {
                self.expr_contains_variable(binary.lhs, name)
                    || self.expr_contains_variable(binary.rhs, name)
            }
            Expression::Parenthesized(parenthesized) => {
                self.expr_contains_variable(parenthesized.expression, name)
            }
            Expression::Call(call) => call
                .get_argument_list()
                .arguments
                .iter()
                .any(|argument| self.expr_contains_variable(argument.value(), name)),
            Expression::ArrayAccess(access) => self.expr_contains_variable(access.array, name),
            _ => false,
        }
    }

    fn eval_call(&mut self, call: &Call<'_>, state: &mut State) -> Eval {
        match call {
            Call::Function(function_call) => {
                let name = function_name(function_call.function);
                let arguments = function_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                let mut args = Vec::new();
                let mut effect = Effect::default();
                for argument in &arguments {
                    let evaluated = self.eval_expr(argument.value(), state);
                    effect = effect.add(evaluated.effect.clone());
                    args.push(evaluated);
                }
                let Some(name) = name else {
                    return Eval {
                        value: ValueState::default(),
                        effect,
                    };
                };
                if is_source_function(&name) {
                    let mut origins = BTreeSet::new();
                    origins.insert(format!("{name}()"));
                    let mut value = ValueState::default();
                    value.tainted = true;
                    value.taint_origins = origins;
                    return Eval { value, effect };
                }
                if name == "file_get_contents" {
                    if let Some(argument) = arguments.first() {
                        if literal_string(argument.value())
                            .is_some_and(|path| path.contains("php://input"))
                        {
                            let mut value = ValueState::string(SizeExpr::Unknown, true);
                            value.taint_origins.insert(String::from("php://input"));
                            return Eval { value, effect };
                        }
                    }
                }
                if let Some(value) =
                    self.eval_builtin(&name, &args, &arguments, function_call.span(), state)
                {
                    return Eval {
                        value: value.value,
                        effect: effect.add(value.effect),
                    };
                }
                if let Some(summary) = self.summaries.get(&name).cloned() {
                    return self.instantiate_summary(
                        &name,
                        &summary,
                        &args,
                        &arguments,
                        function_call.span(),
                        effect,
                        state,
                    );
                }
                let mut value = ValueState::default();
                value.tainted = effect.tainted;
                value.taint_origins = effect.origins.clone();
                Eval { value, effect }
            }
            Call::Method(method_call) => {
                let object = self.eval_expr(method_call.object, state);
                let Some(name) = method_name(&method_call.method) else {
                    return Eval {
                        value: ValueState::default(),
                        effect: object.effect,
                    };
                };
                let arguments = method_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                let mut args = Vec::new();
                let mut effect = object.effect;
                for argument in &arguments {
                    let evaluated = self.eval_expr(argument.value(), state);
                    effect = effect.add(evaluated.effect.clone());
                    args.push(evaluated);
                }
                if is_request_method(&name) && is_request_object(method_call.object) {
                    let mut origins = object.value.taint_origins.clone();
                    if origins.is_empty() {
                        origins.insert(format!("request->{name}()"));
                    }
                    let mut value = ValueState::default();
                    value.tainted = true;
                    value.taint_origins = origins;
                    return Eval { value, effect };
                }
                if let Some(summary) = self.summary_for_method(None, &name).cloned() {
                    return self.instantiate_summary(
                        &name,
                        &summary,
                        &args,
                        &arguments,
                        method_call.span(),
                        effect,
                        state,
                    );
                }
                let mut value = ValueState::default();
                value.tainted = effect.tainted;
                value.taint_origins = effect.origins.clone();
                Eval { value, effect }
            }
            Call::NullSafeMethod(method_call) => {
                let object = self.eval_expr(method_call.object, state);
                let Some(name) = method_name(&method_call.method) else {
                    return Eval {
                        value: ValueState::default(),
                        effect: object.effect,
                    };
                };
                let arguments = method_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                let mut args = Vec::new();
                let mut effect = object.effect;
                for argument in &arguments {
                    let evaluated = self.eval_expr(argument.value(), state);
                    effect = effect.add(evaluated.effect.clone());
                    args.push(evaluated);
                }
                if is_request_method(&name) {
                    let mut value = ValueState::default();
                    value.tainted = true;
                    value.taint_origins.insert(format!("request->{name}()"));
                    return Eval { value, effect };
                }
                if let Some(summary) = self.summary_for_method(None, &name).cloned() {
                    return self.instantiate_summary(
                        &name,
                        &summary,
                        &args,
                        &arguments,
                        method_call.span(),
                        effect,
                        state,
                    );
                }
                let mut value = ValueState::default();
                value.tainted = effect.tainted;
                value.taint_origins = effect.origins.clone();
                Eval { value, effect }
            }
            Call::StaticMethod(static_call) => {
                let class = expression_name(static_call.class);
                let Some(name) = method_name(&static_call.method) else {
                    return Eval::pure(ValueState::default());
                };
                let arguments = static_call
                    .argument_list
                    .arguments
                    .iter()
                    .collect::<Vec<_>>();
                let mut args = Vec::new();
                let mut effect = Effect::default();
                for argument in &arguments {
                    let evaluated = self.eval_expr(argument.value(), state);
                    effect = effect.add(evaluated.effect.clone());
                    args.push(evaluated);
                }
                if let Some(summary) = self.summary_for_method(class.as_deref(), &name).cloned() {
                    return self.instantiate_summary(
                        &name,
                        &summary,
                        &args,
                        &arguments,
                        static_call.span(),
                        effect,
                        state,
                    );
                }
                let mut value = ValueState::default();
                value.tainted = effect.tainted;
                value.taint_origins = effect.origins.clone();
                Eval { value, effect }
            }
        }
    }
}

impl<'a> Analyzer<'a> {
    fn eval_builtin(
        &mut self,
        name: &str,
        args: &[Eval],
        arguments: &[&Argument<'_>],
        span: Span,
        state: &mut State,
    ) -> Option<Eval> {
        let origins = args.iter().fold(BTreeSet::new(), |mut origins, argument| {
            origins.extend(argument.value.taint_origins.clone());
            origins
        });
        let tainted = args.iter().any(|argument| argument.value.tainted);
        let mut effect = Effect::default();

        let value = match name {
            "count" | "sizeof" => {
                let argument = args.first()?.value.clone();
                let mut value = ValueState::scalar();
                value.count = argument.count;
                value.tainted = argument.tainted;
                value.taint_origins = argument.taint_origins;
                value
            }
            "strlen" | "mb_strlen" => {
                let argument = args.first()?.value.clone();
                let mut value = ValueState::scalar();
                value.count = argument.bytes;
                value.tainted = argument.tainted;
                value.taint_origins = argument.taint_origins;
                value
            }
            "array_merge" => {
                let count = args.iter().fold(SizeExpr::Const(0), |total, argument| {
                    total.add(argument.value.count.clone())
                });
                let bytes = args.iter().fold(SizeExpr::Const(0), |total, argument| {
                    total.add(argument.value.bytes.clone())
                });
                let element_bytes = args.iter().fold(SizeExpr::Const(0), |max, argument| {
                    max.max(argument.value.element_bytes.clone())
                });
                let mut value =
                    ValueState::array(count.clone(), bytes.clone(), element_bytes, tainted);
                value.taint_origins = origins.clone();
                value.alloc = args
                    .iter()
                    .fold(CostExpr::Constant, |alloc, argument| {
                        alloc.add(argument.value.alloc.clone())
                    })
                    .add(CostExpr::linear(bytes.clone()));
                value.peak = bytes.clone();
                effect.cost = CostExpr::linear(count);
                effect.alloc = CostExpr::linear(bytes.clone());
                effect.peak = bytes;
                value
            }
            "array_unshift" => {
                let element = args
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| Eval::pure(ValueState::scalar()));
                let first = arguments.first()?;
                let variable = direct_variable_name(first.value())?;
                let variable = variable.to_string();
                let base = self.lookup(&variable, state);
                let mut value = base.clone();
                value.count = value.count.add(SizeExpr::Const(1));
                value.bytes = value.bytes.add(element.value.bytes.clone());
                value.element_bytes = value.element_bytes.max(element.value.bytes.clone());
                value.alloc = value.alloc.add(CostExpr::linear(value.bytes.clone()));
                value.peak = value.bytes.clone().max(base.peak.clone());
                value.tainted = base.tainted || element.value.tainted;
                value.taint_origins = base
                    .taint_origins
                    .union(&element.value.taint_origins)
                    .cloned()
                    .collect();
                state.env.insert(variable.clone(), value.clone());
                effect.cost = CostExpr::linear(base.count.clone());
                effect.alloc = CostExpr::linear(value.bytes.clone());
                effect.retained_delta = element.value.bytes.clone();
                effect.peak = value.bytes.clone();
                effect.tainted = value.tainted;
                effect.origins = value.taint_origins.clone();
                effect
                    .growth
                    .insert(variable, Growth::unshift(element.value.bytes.clone()));
                let mut scalar = ValueState::scalar();
                scalar.count = value.count.clone();
                scalar.tainted = value.tainted;
                scalar.taint_origins = value.taint_origins.clone();
                scalar
            }
            "array_push" => {
                let first = arguments.first()?;
                let variable = direct_variable_name(first.value())?;
                let variable = variable.to_string();
                let base = self.lookup(&variable, state);
                let added_count = SizeExpr::Const((args.len().saturating_sub(1)) as u64);
                let added_bytes = args
                    .iter()
                    .skip(1)
                    .fold(SizeExpr::Const(0), |total, argument| {
                        total.add(argument.value.bytes.clone())
                    });
                let mut value = base.clone();
                value.count = value.count.add(added_count);
                value.bytes = value.bytes.add(added_bytes.clone());
                value.element_bytes = value.element_bytes.max(added_bytes.clone());
                value.alloc = value.alloc.add(CostExpr::linear(value.bytes.clone()));
                value.peak = value.bytes.clone().max(base.peak.clone());
                value.tainted = base.tainted || tainted;
                value.taint_origins = base.taint_origins.union(&origins).cloned().collect();
                state.env.insert(variable.clone(), value.clone());
                effect.alloc = CostExpr::linear(added_bytes.clone());
                effect.retained_delta = added_bytes.clone();
                effect.peak = value.bytes.clone();
                effect.tainted = value.tainted;
                effect.origins = value.taint_origins.clone();
                effect.growth.insert(variable, Growth::append(added_bytes));
                let mut scalar = ValueState::scalar();
                scalar.count = value.count.clone();
                scalar.tainted = value.tainted;
                scalar.taint_origins = value.taint_origins.clone();
                scalar
            }
            "array_pop" => {
                let first = arguments.first()?;
                let variable = direct_variable_name(first.value())?;
                let variable = variable.to_string();
                let base = self.lookup(&variable, state);
                let mut value = base.clone();
                value.count = value.count.clone();
                value.bytes = SizeExpr::Unknown;
                value.peak = base.peak.clone();
                state.env.insert(variable, value);
                let mut element = ValueState::scalar();
                element.bytes = base.element_bytes.clone();
                element.tainted = base.tainted;
                element.taint_origins = base.taint_origins;
                element
            }
            "array_shift" => {
                let first = arguments.first()?;
                let variable = direct_variable_name(first.value())?;
                let variable = variable.to_string();
                let base = self.lookup(&variable, state);
                effect.cost = CostExpr::linear(base.count.clone());
                let mut value = base.clone();
                value.bytes = SizeExpr::Unknown;
                state.env.insert(variable, value);
                let mut element = ValueState::scalar();
                element.bytes = base.element_bytes.clone();
                element.tainted = base.tainted;
                element.taint_origins = base.taint_origins;
                element
            }
            "array_slice" => {
                let argument = args.first()?.value.clone();
                let limit = arguments
                    .get(2)
                    .and_then(|argument| literal_u64(argument.value()));
                let count = match limit {
                    Some(limit) => argument.count.clone().min(SizeExpr::Const(limit)),
                    None => argument.count.clone(),
                };
                let bytes = argument.element_bytes.clone().mul(count.clone());
                let mut value = ValueState::array(
                    count.clone(),
                    bytes.clone(),
                    argument.element_bytes.clone(),
                    tainted,
                );
                value.taint_origins = origins.clone();
                effect.cost = CostExpr::linear(argument.count.clone());
                value
            }
            "array_values" | "array_reverse" | "array_filter" | "array_unique" | "array_keys"
            | "array_flip" | "array_chunk" | "array_combine" => {
                let argument = args.first()?.value.clone();
                let mut value = argument.clone();
                value.taint_origins = origins.clone();
                value.tainted = tainted;
                effect.cost = CostExpr::linear(argument.count.clone());
                value
            }
            "array_map" => {
                let count = args.iter().fold(SizeExpr::Const(0), |max, argument| {
                    max.max(argument.value.count.clone())
                });
                let mut value =
                    ValueState::array(count.clone(), SizeExpr::Unknown, SizeExpr::Unknown, tainted);
                value.taint_origins = origins.clone();
                effect.cost = CostExpr::linear(count);
                value
            }
            "str_repeat" => {
                let argument = args.first()?.value.clone();
                let repeat = args
                    .get(1)
                    .map(|argument| argument.value.count.clone())
                    .unwrap_or(SizeExpr::Const(1));
                let bytes = argument.bytes.clone().mul(repeat);
                let mut value = ValueState::string(bytes.clone(), tainted);
                value.taint_origins = origins.clone();
                value.alloc = CostExpr::linear(bytes.clone());
                effect.cost = CostExpr::linear(bytes.clone());
                effect.alloc = CostExpr::linear(bytes.clone());
                effect.peak = bytes;
                value
            }
            "range" => {
                let count = range_count(args, arguments);
                let bytes = count.clone().mul_const(8);
                let mut value =
                    ValueState::array(count.clone(), bytes.clone(), SizeExpr::Const(8), tainted);
                value.taint_origins = origins.clone();
                value.alloc = CostExpr::linear(bytes.clone());
                effect.cost = CostExpr::linear(count);
                effect.alloc = CostExpr::linear(bytes.clone());
                effect.peak = bytes;
                value
            }
            "array_fill" => {
                let count = args.get(1)?.value.count.clone();
                let bytes = count.clone().mul(args.get(2)?.value.bytes.clone());
                let mut value = ValueState::array(
                    count.clone(),
                    bytes.clone(),
                    args.get(2)?.value.bytes.clone(),
                    tainted,
                );
                value.taint_origins = origins.clone();
                value.alloc = CostExpr::linear(bytes.clone());
                effect.cost = CostExpr::linear(count);
                effect.alloc = CostExpr::linear(bytes.clone());
                effect.peak = bytes;
                value
            }
            "array_pad" => {
                let argument = args.first()?.value.clone();
                let count = argument.count.clone().max(
                    args.get(1)
                        .map(|argument| argument.value.count.clone())
                        .unwrap_or(SizeExpr::Unknown),
                );
                let mut value =
                    ValueState::array(count, SizeExpr::Unknown, argument.element_bytes, tainted);
                value.taint_origins = origins.clone();
                effect.cost = CostExpr::linear(argument.count);
                value
            }
            "file_get_contents" => {
                let path = arguments
                    .first()
                    .and_then(|argument| literal_string(argument.value()));
                let bytes = path
                    .as_deref()
                    .map(SizeExpr::file_size)
                    .unwrap_or(SizeExpr::Unknown);
                let mut value = ValueState::string(bytes.clone(), tainted);
                value.taint_origins = origins.clone();
                value.alloc = CostExpr::linear(bytes.clone());
                effect.cost = CostExpr::linear(bytes.clone());
                effect.alloc = CostExpr::linear(bytes.clone());
                effect.peak = bytes;
                value
            }
            "fopen" => {
                let mut value = ValueState::default();
                value.stream = true;
                value.count = SizeExpr::Const(0);
                value.bytes = SizeExpr::Const(0);
                value.tainted = tainted;
                value.taint_origins = origins.clone();
                value
            }
            "fgets" | "fgetcsv" | "fgetss" => {
                let argument = args.first()?.value.clone();
                let mut value = ValueState::string(SizeExpr::Unknown, false);
                value.tainted = argument.tainted;
                value.taint_origins = argument.taint_origins;
                value.stream = true;
                value
            }
            "fread" => {
                let bytes = args
                    .get(1)
                    .map(|argument| argument.value.count.clone())
                    .unwrap_or(SizeExpr::Unknown);
                let mut value = ValueState::string(bytes.clone(), tainted);
                value.taint_origins = origins.clone();
                value.alloc = CostExpr::linear(bytes.clone());
                effect.cost = CostExpr::linear(bytes.clone());
                effect.alloc = CostExpr::linear(bytes.clone());
                effect.peak = bytes;
                value
            }
            "stream_get_contents" => {
                let mut value = ValueState::string(SizeExpr::Unknown, tainted);
                value.taint_origins = origins.clone();
                value
            }
            "file" => {
                let mut value = ValueState::array(
                    SizeExpr::Unknown,
                    SizeExpr::Unknown,
                    SizeExpr::Unknown,
                    tainted,
                );
                value.taint_origins = origins.clone();
                effect.cost = CostExpr::linear(SizeExpr::Unknown);
                value
            }
            "json_decode" | "unserialize" | "simplexml_load_string" | "iterator_to_array" => {
                let mut value = ValueState::default();
                value.tainted = tainted;
                value.taint_origins = origins.clone();
                value
            }
            "min" => {
                let value =
                    args.iter()
                        .map(|argument| argument.value.clone())
                        .reduce(|left, right| ValueState {
                            count: left.count.min(right.count),
                            bytes: left.bytes.min(right.bytes),
                            element_bytes: left.element_bytes.min(right.element_bytes),
                            alloc: left.alloc.add(right.alloc),
                            peak: left.peak.max(right.peak),
                            tainted: left.tainted || right.tainted,
                            taint_origins: left
                                .taint_origins
                                .union(&right.taint_origins)
                                .cloned()
                                .collect(),
                            stream: left.stream && right.stream,
                            scalar: left.scalar && right.scalar,
                            growth: None,
                        })?;
                value
            }
            "max" => {
                let value =
                    args.iter()
                        .map(|argument| argument.value.clone())
                        .reduce(|left, right| ValueState {
                            count: left.count.max(right.count),
                            bytes: left.bytes.max(right.bytes),
                            element_bytes: left.element_bytes.max(right.element_bytes),
                            alloc: left.alloc.add(right.alloc),
                            peak: left.peak.max(right.peak),
                            tainted: left.tainted || right.tainted,
                            taint_origins: left
                                .taint_origins
                                .union(&right.taint_origins)
                                .cloned()
                                .collect(),
                            stream: left.stream && right.stream,
                            scalar: left.scalar && right.scalar,
                            growth: None,
                        })?;
                value
            }
            "intval"
            | "floatval"
            | "abs"
            | "round"
            | "floor"
            | "ceil"
            | "array_sum"
            | "array_product"
            | "array_key_exists"
            | "in_array"
            | "preg_match"
            | "preg_match_all"
            | "memory_get_peak_usage"
            | "memory_get_usage"
            | "hrtime"
            | "microtime"
            | "time" => {
                let mut value = ValueState::scalar();
                value.count = args
                    .first()
                    .map(|argument| argument.value.count.clone())
                    .unwrap_or(SizeExpr::Const(1));
                value.tainted = tainted;
                value.taint_origins = origins.clone();
                value
            }
            "implode" | "join" => {
                let argument = args.last()?.value.clone();
                let mut value = ValueState::string(argument.bytes.clone(), tainted);
                value.taint_origins = origins.clone();
                effect.cost = CostExpr::linear(argument.bytes);
                value
            }
            "str_pad" | "substr" | "trim" | "ltrim" | "rtrim" | "strtolower" | "strtoupper"
            | "ucfirst" | "ucwords" | "str_replace" | "preg_replace" => {
                let argument = args.first()?.value.clone();
                let mut value = ValueState::string(argument.bytes.clone(), tainted);
                value.taint_origins = origins.clone();
                effect.cost = CostExpr::linear(argument.bytes);
                value
            }
            "sprintf" | "printf" | "vsprintf" | "number_format" => {
                let mut value = ValueState::string(SizeExpr::Unknown, tainted);
                value.taint_origins = origins.clone();
                value
            }
            "sort"
            | "rsort"
            | "usort"
            | "uasort"
            | "uksort"
            | "shuffle"
            | "array_multisort"
            | "array_walk"
            | "array_walk_recursive" => {
                if let Some(first) = arguments.first() {
                    if let Some(variable) = direct_variable_name(first.value()) {
                        if let Some(value) = state.env.get_mut(variable) {
                            value.peak = value.bytes.clone();
                        }
                    }
                }
                effect.cost = CostExpr::linear(
                    args.first()
                        .map(|argument| argument.value.count.clone())
                        .unwrap_or(SizeExpr::Unknown),
                );
                ValueState::scalar()
            }
            "fwrite" | "fputs" | "fprintf" => {
                effect.cost = CostExpr::linear(
                    args.get(1)
                        .map(|argument| argument.value.bytes.clone())
                        .unwrap_or(SizeExpr::Unknown),
                );
                ValueState::scalar()
            }
            "fclose" | "fflush" | "rewind" | "fseek" => ValueState::scalar(),
            _ => return None,
        };

        self.report_input_sink(name, args, span);

        Some(Eval { value, effect })
    }

    fn report_input_sink(&mut self, name: &str, args: &[Eval], span: Span) {
        if !is_input_sink(name) {
            return;
        }
        let tainted = args.iter().find(|argument| argument.value.tainted);
        let Some(argument) = tainted else {
            return;
        };
        let size = &argument.value.count;
        if size.is_bounded() {
            return;
        }
        let source = argument
            .value
            .taint_origins
            .iter()
            .next()
            .cloned()
            .unwrap_or_else(|| String::from("request input"));
        self.report(
            ResourceClass::InputCardinality,
            span,
            "E0046:input-cardinality",
            "Input-controlled cardinality from {source} reaches {sink}; the result can grow without a bound. Reject or cap the input before allocating.",
            &[
                ("source", source),
                ("sink", format!("{name}()")),
            ],
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn instantiate_summary(
        &mut self,
        name: &str,
        summary: &ResourceSummary,
        args: &[Eval],
        arguments: &[&Argument<'_>],
        span: Span,
        mut effect: Effect,
        state: &State,
    ) -> Eval {
        let count_map = self.summary_replacements(&summary.params, arguments, state, false);
        let bytes_map = self.summary_replacements(&summary.params, arguments, state, true);

        let mut cost = summary.cost.substitute(&count_map);
        if summary.recursive {
            self.recursive = true;
            cost = cost.add(CostExpr::Exponential(
                summary
                    .params
                    .first()
                    .map(|param| SizeExpr::Param(param.clone()))
                    .unwrap_or(SizeExpr::Unknown),
            ));
        }
        let alloc = summary.heap.alloc.substitute(&bytes_map);
        let retained = summary.heap.retained.substitute(&bytes_map);
        let peak = summary.heap.peak.substitute(&bytes_map);

        let mut value = ValueState {
            count: summary.return_count.substitute(&count_map),
            bytes: summary.return_bytes.substitute(&bytes_map),
            element_bytes: SizeExpr::Unknown,
            alloc: alloc.clone(),
            peak: peak.clone(),
            tainted: false,
            taint_origins: BTreeSet::new(),
            stream: false,
            scalar: false,
            growth: None,
        };
        for index in &summary.return_tainted_from {
            if let Some(argument) = args.get(*index) {
                if argument.value.tainted {
                    value.tainted = true;
                    value
                        .taint_origins
                        .extend(argument.value.taint_origins.clone());
                }
            }
        }
        if value.tainted && value.taint_origins.is_empty() {
            value.taint_origins.insert(format!("{name}()"));
        }

        for (index, param) in summary.params.iter().enumerate() {
            if !summary.sensitive_params.contains(param) {
                continue;
            }
            let Some(argument) = args.get(index) else {
                continue;
            };
            if argument.value.tainted && !argument.value.count.is_bounded() {
                let source = argument
                    .value
                    .taint_origins
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| String::from("request input"));
                self.report(
                    ResourceClass::InputCardinality,
                    span,
                    "E0046:input-cardinality",
                    "Input-controlled cardinality from {source} reaches {sink}; the callee iterates or allocates in proportion to it. Reject or cap the input before the call.",
                    &[
                        ("source", source),
                        ("sink", format!("{name}()")),
                    ],
                );
            }
            for current_param in &self.params {
                if self
                    .size_of_expr(arguments[index].value(), state)
                    .contains_param(current_param)
                {
                    self.sensitive_params.insert(current_param.clone());
                }
            }
        }

        effect.cost = effect.cost.add(cost);
        effect.alloc = effect.alloc.add(alloc);
        effect.retained_delta = effect.retained_delta.add(retained);
        effect.peak = effect.peak.max(peak);
        effect.retains |= !summary.heap.retained.is_bounded();
        effect.tainted |= value.tainted;
        effect.origins.extend(value.taint_origins.clone());

        Eval { value, effect }
    }
}

fn range_count(args: &[Eval], arguments: &[&Argument<'_>]) -> SizeExpr {
    let start = arguments
        .first()
        .and_then(|argument| literal_u64(argument.value()));
    let end = arguments
        .get(1)
        .and_then(|argument| literal_u64(argument.value()));
    if let (Some(start), Some(end)) = (start, end) {
        let count = if end >= start {
            end.saturating_sub(start).saturating_add(1)
        } else {
            start.saturating_sub(end).saturating_add(1)
        };
        return SizeExpr::Const(count);
    }
    args.get(1)
        .map(|argument| argument.value.count.clone())
        .unwrap_or(SizeExpr::Unknown)
}

fn is_input_sink(name: &str) -> bool {
    matches!(
        name,
        "str_repeat"
            | "range"
            | "array_fill"
            | "array_pad"
            | "file_get_contents"
            | "file"
            | "str_split"
            | "explode"
            | "preg_split"
            | "array_fill_keys"
            | "iterator_to_array"
    )
}
