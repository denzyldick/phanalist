use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::{Add, Mul, Sub};

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;
use z3::ast::{Bool, Float, Int, RoundingMode};
use z3::{Params, SatResult, Solver};

use crate::file::File;
use crate::results::Message;

use super::contracts::{self, Docblock};
use super::summary::{ConstantValue, FunctionSummary};
use super::value::{ArrayKey, Value};
use super::Settings;

pub struct Finding {
    pub message: Message,
    pub span: Span,
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
}

struct Analyzer<'a, 'arena> {
    #[allow(dead_code)]
    file: &'a File<'arena>,
    settings: &'a Settings,
    findings: Vec<Finding>,
    seen: HashSet<(u32, u32, &'static str)>,
    return_hint: Option<&'arena Hint<'arena>>,
    strict_types: bool,
    finding_limit_start: usize,
    suppress_access_checks: bool,
    summaries: &'a HashMap<String, FunctionSummary>,
    namespace: Option<String>,
    class: Option<String>,
    switch_depth: usize,
    docblocks: Vec<Trivia<'arena>>,
    doc_return: Option<String>,
}

pub fn analyze_file_with_summaries(
    file: &File<'_>,
    settings: &Settings,
    summaries: &HashMap<String, FunctionSummary>,
) -> Vec<Finding> {
    let Some(program) = file.ast else {
        return Vec::new();
    };

    let mut analyzer = Analyzer {
        file,
        settings,
        findings: Vec::new(),
        seen: HashSet::new(),
        return_hint: None,
        strict_types: file_declares_strict_types(program),
        finding_limit_start: 0,
        suppress_access_checks: false,
        summaries,
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
    };

    let mut state = State::new();
    for statement in program.statements.iter() {
        analyzer.analyze_statement(statement, &mut state);
    }

    analyzer.findings
}

impl<'a, 'arena> Analyzer<'a, 'arena> {
    fn analyze_statement(&mut self, statement: &'arena Statement<'arena>, state: &mut State) {
        if !state.reachable {
            if !is_declaration(statement) {
                if self.settings.check_unreachable {
                    self.push_finding(
                        "unreachable",
                        "This statement is unreachable.",
                        statement.span(),
                        "?",
                    );
                }
                return;
            }
        }

        match statement {
            Statement::Block(block) => self.analyze_statements(block.statements.as_slice(), state),
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
            Statement::DoWhile(do_while) => {
                let mut body_state = state.clone();
                self.analyze_statement(do_while.statement, &mut body_state);
                self.eval_expr(do_while.condition, &mut body_state);
                self.havoc_modified(state, &body_state);
            }
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
            Statement::Try(try_statement) => {
                self.analyze_statements(try_statement.block.statements.as_slice(), state);
                for catch in try_statement.catch_clauses.iter() {
                    let mut catch_state = state.clone();
                    self.analyze_statements(catch.block.statements.as_slice(), &mut catch_state);
                }
                if let Some(finally) = &try_statement.finally_clause {
                    self.analyze_statements(finally.block.statements.as_slice(), state);
                }
            }
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
        self.class = Some(self.qualified_name(&name.to_ascii_lowercase()));
        self.analyze_members(members);
        self.class = previous;
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
        let previous_limit = self.finding_limit_start;
        self.return_hint = return_hint.map(|hint| &hint.hint);
        self.finding_limit_start = self.findings.len();

        let docblock = self.docblock_before(span).unwrap_or_default();
        self.doc_return = docblock.return_type.clone();

        let mut state = State::new();
        for parameter in parameters.parameters.iter() {
            let parameter_name = String::from_utf8_lossy(parameter.variable.name).to_string();
            let doc_type = docblock
                .params
                .get(parameter_name.strip_prefix('$').unwrap_or(&parameter_name));
            let value = parameter_value(
                parameter.hint.as_ref(),
                doc_type.map(String::as_str),
                name,
                &parameter_name,
            );
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
        self.finding_limit_start = previous_limit;
    }

    fn apply_docblock_vars(
        &self,
        docblock: &Docblock,
        state: &mut State,
        assignment: Option<&str>,
    ) {
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
            merged.modified.insert(name);
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

        self.eval_expr(foreach.expression, state);
        let mut body_state = state.clone();
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
        self.havoc_modified(state, &body_state);
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
            state.env.insert(name.clone(), Value::Unknown);
            state.modified.insert(name.clone());
            state.array_values.remove(&name);
        }
    }

    fn analyze_switch(&mut self, switch: &'arena Switch<'arena>, state: &mut State) {
        let subject = self.eval_expr(switch.expression, state);
        let has_default = switch.body.has_default_case();
        let mut pending: Vec<Bool> = Vec::new();
        let mut known_conditions: Vec<Bool> = Vec::new();
        let mut has_unknown_condition = false;
        let mut branch_states = Vec::new();

        self.switch_depth += 1;
        for case in switch.body.cases() {
            let case_condition = case.expression().map(|expression| {
                let value = self.eval_expr(expression, state);
                self.compare_identical(&subject, &value, false).truthiness()
            });

            if let Some(Some(condition)) = &case_condition {
                known_conditions.push(condition.clone());
            } else if !case.is_default() {
                has_unknown_condition = true;
            }

            if case.is_empty() {
                if let Some(Some(condition)) = case_condition {
                    pending.push(condition);
                }
                continue;
            }

            let condition = match case_condition.flatten() {
                Some(condition) => {
                    pending.push(condition);
                    Some(path_or_many(&pending))
                }
                None => None,
            };
            pending.clear();

            let mut branch_state = state.clone();
            if let Some(condition) = &condition {
                branch_state.path = path_and(&state.path, condition);
            }
            self.analyze_statements(case.statements(), &mut branch_state);
            branch_states.push(branch_state);
        }
        self.switch_depth -= 1;

        if !has_default {
            let mut fallthrough = state.clone();
            if !has_unknown_condition {
                let any_case = path_or_many(&known_conditions);
                fallthrough.path = path_and(&state.path, &path_not(&any_case));
            }
            branch_states.push(fallthrough);
        }

        if branch_states.is_empty() {
            return;
        }

        let mut merged = state.clone();
        let mut modified = HashSet::new();
        for branch_state in &branch_states {
            modified.extend(branch_state.modified.iter().cloned());
        }
        for name in modified {
            merged.env.insert(name.clone(), Value::Unknown);
            merged.modified.insert(name);
        }

        let mut paths = Vec::new();
        for branch_state in &branch_states {
            paths.push(branch_state.path.clone());
            if !matches!(branch_state.switch_exits.as_bool(), Some(false)) {
                paths.push(branch_state.switch_exits.clone());
            }
        }
        merged.path = path_or_many(&paths);
        merged.switch_exits = Bool::from_bool(false);
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
        merged.reachable = branch_states.iter().any(|branch| {
            branch.reachable
                || (!matches!(branch.switch_exits.as_bool(), Some(false))
                    && path_may_be_true(&branch.switch_exits))
        });
        *state = merged;
    }

    fn analyze_loop(
        &mut self,
        condition: Option<&'arena Expression<'arena>>,
        body: &'arena [Statement<'arena>],
        increments: Option<&'arena [&'arena Expression<'arena>]>,
        state: &mut State,
    ) -> State {
        let depth = self.loop_unroll_depth();
        let mut current = state.clone();
        let mut exit_paths = Vec::new();
        let mut reachable = false;
        let mut modified = state.modified.clone();

        for _ in 0..depth {
            let condition_value =
                condition.map(|condition| self.eval_expr(condition, &mut current));
            let condition = condition_value.as_ref().and_then(Value::truthiness);

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

        for name in modified {
            state.env.insert(name.clone(), Value::Unknown);
            state.modified.insert(name);
        }
        state.path = path_or_many(&exit_paths);
        state.reachable = reachable;

        current
    }

    fn loop_unroll_depth(&self) -> usize {
        if self.settings.loop_strategy == "none" {
            return 1;
        }

        self.settings.unroll_depth.clamp(1, 64)
    }

    fn havoc_modified(&self, state: &mut State, body_state: &State) {
        for name in &body_state.modified {
            state.env.insert(name.clone(), Value::Unknown);
            state.modified.insert(name.clone());
        }
    }

    fn eval_expr(&mut self, expression: &'arena Expression<'arena>, state: &mut State) -> Value {
        match expression {
            Expression::Parenthesized(parenthesized) => {
                self.eval_expr(parenthesized.expression, state)
            }
            Expression::Literal(literal) => self.eval_literal(literal),
            Expression::Variable(variable) => self.eval_variable(variable, state),
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
            Expression::Instantiation(instantiation) => {
                if let Some(arguments) = &instantiation.argument_list {
                    self.eval_arguments(arguments, state);
                }
                Value::Unknown
            }
            Expression::Clone(clone) => self.eval_expr(clone.object, state),
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
                Value::Unknown
            }
            Expression::Access(access) => match access {
                Access::Property(access) => {
                    let object = self.eval_expr(access.object, state);
                    if self.settings.check_nullability
                        && !self.suppress_access_checks
                        && matches!(object, Value::Null)
                    {
                        self.push_finding(
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
        let mut branches = Vec::new();

        for arm in match_.arms.iter() {
            match arm {
                MatchArm::Default(arm) => {
                    fallback = self.eval_expr(arm.expression, state);
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
                    for name in &branch_state.modified {
                        state.env.insert(name.clone(), Value::Unknown);
                        state.modified.insert(name.clone());
                    }
                    branches.push((condition, value));
                }
            }
        }

        let mut result = fallback;
        for (condition, value) in branches.into_iter().rev() {
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
                if self.settings.check_modulo && matches!(right, Value::Int(_)) {
                    self.check_nonzero(
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
                        _ => None,
                    },
                }
            } else {
                None
            };
            state.assign(name.clone(), value.clone());
            if let Some(pairs) = array_value {
                state.array_values.insert(name, pairs);
            }
        }

        if assigned_name.is_none() {
            if let Some(root) = root_variable_name(assignment.lhs) {
                state.array_values.remove(&root);
            }
        }
        value
    }

    fn eval_binary(&mut self, binary: &'arena Binary<'arena>, state: &mut State) -> Value {
        if matches!(
            binary.operator,
            BinaryOperator::And(_) | BinaryOperator::LowAnd(_)
        ) {
            let left = self.eval_expr(binary.lhs, state);
            if let Some(left) = left.truthiness() {
                if left.as_bool() == Some(false) {
                    return Value::Bool(Bool::from_bool(false));
                }
            }
            let right = self.eval_expr(binary.rhs, state);
            return match (left.truthiness(), right.truthiness()) {
                (Some(left), Some(right)) => Value::Bool(Bool::and(&[&left, &right])),
                _ => Value::Unknown,
            };
        }

        if matches!(
            binary.operator,
            BinaryOperator::Or(_) | BinaryOperator::LowOr(_)
        ) {
            let left = self.eval_expr(binary.lhs, state);
            if let Some(left) = left.truthiness() {
                if left.as_bool() == Some(true) {
                    return Value::Bool(Bool::from_bool(true));
                }
            }
            let right = self.eval_expr(binary.rhs, state);
            return match (left.truthiness(), right.truthiness()) {
                (Some(left), Some(right)) => Value::Bool(Bool::or(&[&left, &right])),
                _ => Value::Unknown,
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
        let right = self.eval_expr(binary.rhs, state);

        match binary.operator {
            BinaryOperator::Division(_) => {
                if self.settings.check_division {
                    self.check_nonzero(
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
                if self.settings.check_modulo && matches!(right, Value::Int(_)) {
                    self.check_nonzero(
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
                .map(|value| Value::Bool(value.not()))
                .unwrap_or(Value::Unknown),
            UnaryPrefixOperator::Plus(_) => self.eval_expr(unary.operand, state),
            UnaryPrefixOperator::Negation(_) => match self.eval_expr(unary.operand, state) {
                Value::Int(value) => Value::Int(value.unary_minus()),
                Value::Float(value) => Value::Float(value.unary_neg()),
                _ => Value::Unknown,
            },
            UnaryPrefixOperator::BitwiseNot(_) => Value::Unknown,
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

        let mut then_state = state.clone();
        if let Some(condition) = &condition {
            then_state.path = path_and(&state.path, condition);
        }
        let then_value = match conditional.then {
            Some(value) => self.eval_expr(value, &mut then_state),
            None => condition_value.clone(),
        };

        let mut else_state = state.clone();
        if let Some(condition) = &condition {
            else_state.path = path_and(&state.path, &path_not(condition));
        }
        let else_value = self.eval_expr(conditional.r#else, &mut else_state);

        if let Some(condition) = condition {
            Value::merge(Some(&condition), &then_value, &else_value)
        } else {
            Value::Unknown
        }
    }

    fn function_summary(&self, expression: &Expression<'_>) -> Option<&FunctionSummary> {
        let name = function_identifier_name(expression)?;
        if name.contains('\\') {
            return self.summaries.get(&name);
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
            Expression::Identifier(identifier) => {
                let name = String::from_utf8_lossy(identifier.value()).to_ascii_lowercase();
                match identifier {
                    Identifier::FullyQualified(_) => {
                        Some(name.trim_start_matches('\\').to_string())
                    }
                    Identifier::Local(_) => Some(self.qualified_name(&name)),
                    Identifier::Qualified(_) => Some(self.qualified_name(&name)),
                }
            }
            Expression::Self_(_) | Expression::Static(_) => self.class.clone(),
            _ => None,
        }
    }

    fn eval_call(&mut self, call: &'arena Call<'arena>, state: &mut State) -> Value {
        match call {
            Call::Function(function) => {
                let arguments = self.eval_arguments(&function.argument_list, state);
                let name = function_name(function.function);
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
                    } else if self.settings.unknown_policy == "report" {
                        self.push_finding(
                            "unverified",
                            "Could not verify this assertion: its condition is unknown.",
                            call.span(),
                            "?",
                        );
                    }
                }
                if let Some(value) = builtin_call_value(name.as_deref(), &arguments) {
                    return value;
                }
                if let Some(summary) = self.function_summary(function.function) {
                    self.apply_call_assertions(summary, &arguments, &function.argument_list, state);
                    if let Some(value) = summary_call_value(
                        summary,
                        &arguments,
                        arguments_are_positional(&function.argument_list),
                    ) {
                        return value;
                    }
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
                        "null-method-call",
                        "Method call on null.",
                        method.object.span(),
                        "null",
                    );
                }
                let arguments = self.eval_arguments(&method.argument_list, state);
                let class = if direct_variable_name(method.object).as_deref() == Some("$this") {
                    self.class.as_deref()
                } else {
                    None
                };
                if let Some(summary) =
                    class.and_then(|class| self.method_summary(class, &method.method))
                {
                    self.apply_call_assertions(summary, &arguments, &method.argument_list, state);
                    if let Some(value) = summary_call_value(
                        summary,
                        &arguments,
                        arguments_are_positional(&method.argument_list),
                    ) {
                        return value;
                    }
                }
                Value::Unknown
            }
            Call::NullSafeMethod(method) => {
                self.eval_expr(method.object, state);
                let arguments = self.eval_arguments(&method.argument_list, state);
                let class = if direct_variable_name(method.object).as_deref() == Some("$this") {
                    self.class.as_deref()
                } else {
                    None
                };
                if let Some(summary) =
                    class.and_then(|class| self.method_summary(class, &method.method))
                {
                    self.apply_call_assertions(summary, &arguments, &method.argument_list, state);
                    if let Some(value) = summary_call_value(
                        summary,
                        &arguments,
                        arguments_are_positional(&method.argument_list),
                    ) {
                        return value;
                    }
                }
                Value::Unknown
            }
            Call::StaticMethod(method) => {
                let class = self.static_class_name(method.class);
                self.eval_expr(method.class, state);
                let arguments = self.eval_arguments(&method.argument_list, state);
                if let Some(summary) = class
                    .as_deref()
                    .and_then(|class| self.method_summary(class, &method.method))
                {
                    self.apply_call_assertions(summary, &arguments, &method.argument_list, state);
                    if let Some(value) = summary_call_value(
                        summary,
                        &arguments,
                        arguments_are_positional(&method.argument_list),
                    ) {
                        return value;
                    }
                }
                Value::Unknown
            }
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
            let value = if !assertion.negated && matches!(value, Value::Unknown) {
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

    fn eval_construct(&mut self, construct: &'arena Construct<'arena>, state: &mut State) -> Value {
        match construct {
            Construct::Isset(isset) => {
                let previous = self.suppress_access_checks;
                self.suppress_access_checks = true;
                for value in isset.values.iter() {
                    self.eval_expr(value, state);
                }
                self.suppress_access_checks = previous;
                Value::Unknown
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
        let mut next_index = 0_i64;
        let mut known = true;

        for element in elements {
            match element {
                ArrayElement::KeyValue(element) => {
                    let key_value = self.eval_expr(element.key, state);
                    self.eval_expr(element.value, state);
                    match array_key(&key_value) {
                        Some(key) => {
                            if let ArrayKey::Int(index) = &key {
                                next_index = next_index.max(index.saturating_add(1));
                            }
                            keys.insert(key);
                        }
                        None => known = false,
                    }
                }
                ArrayElement::Value(element) => {
                    self.eval_expr(element.value, state);
                    keys.insert(ArrayKey::Int(next_index));
                    next_index = next_index.saturating_add(1);
                }
                ArrayElement::Variadic(element) => {
                    self.eval_expr(element.value, state);
                    known = false;
                }
                ArrayElement::Missing(_) => {
                    next_index = next_index.saturating_add(1);
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
        let mut next_index = 0_i64;

        for element in elements {
            match element {
                ArrayElement::KeyValue(element) => {
                    let key = self.eval_expr(element.key, state);
                    let value = self.eval_expr(element.value, state);
                    if let ArrayKey::Int(index) = array_key(&key)? {
                        next_index = next_index.max(index.saturating_add(1));
                    }
                    pairs.push((key, value));
                }
                ArrayElement::Value(element) => {
                    let value = self.eval_expr(element.value, state);
                    pairs.push((Value::Int(Int::from_i64(next_index)), value));
                    next_index = next_index.saturating_add(1);
                }
                ArrayElement::Missing(_) => next_index = next_index.saturating_add(1),
                ArrayElement::Variadic(_) => return None,
            }
        }

        Some(pairs)
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
                        if !keys.contains(&key) {
                            self.push_finding(
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
                if let Value::Int(index) = index {
                    let length = Int::from_i64(string.len() as i64);
                    let out_of_bounds = Bool::or(&[&index.lt(-length.clone()), &index.ge(length)]);
                    let index_value = Value::Int(index.clone());
                    self.check_condition(
                        &out_of_bounds,
                        path,
                        span,
                        "string-offset",
                        "String offset {value} is out of bounds.",
                        Some(&index_value),
                    );
                }
            }
            _ => {}
        }
    }

    fn add_values(&self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => Value::Int(left.add(right)),
            (Value::Float(left), Value::Float(right)) => Value::Float(
                left.add_with_rounding_mode(right, &RoundingMode::round_nearest_ties_to_even()),
            ),
            _ => {
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

    fn sub_values(&self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => Value::Int(left.sub(right)),
            (Value::Float(left), Value::Float(right)) => Value::Float(
                left.sub_with_rounding_mode(right, &RoundingMode::round_nearest_ties_to_even()),
            ),
            _ => {
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

    fn mul_values(&self, left: &Value, right: &Value) -> Value {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => Value::Int(left.mul(right)),
            (Value::Float(left), Value::Float(right)) => Value::Float(
                left.mul_with_rounding_mode(right, &RoundingMode::round_nearest_ties_to_even()),
            ),
            _ => {
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
            _ => Value::Unknown,
        }
    }

    fn compare_equal(&self, left: &Value, right: &Value, negate: bool) -> Value {
        let comparison = match (left, right) {
            (Value::Int(left), Value::Int(right)) => Some(left.eq(right)),
            (Value::Float(left), Value::Float(right)) => Some(left.eq_fpa(right)),
            (Value::Int(left), Value::Float(right)) => Some(left.to_real().eq(&right.to_real())),
            (Value::Float(left), Value::Int(right)) => Some(left.to_real().eq(&right.to_real())),
            (Value::Bool(left), Value::Bool(right)) => Some(left.eq(right)),
            (Value::Null, Value::Null) => Some(Bool::from_bool(true)),
            _ => None,
        };
        comparison
            .map(|comparison| Value::Bool(if negate { comparison.not() } else { comparison }))
            .unwrap_or(Value::Unknown)
    }

    fn compare_identical(&self, left: &Value, right: &Value, negate: bool) -> Value {
        let comparison = match (left, right) {
            (Value::Int(left), Value::Int(right)) => Some(left.eq(right)),
            (Value::Float(left), Value::Float(right)) => Some(left.eq_fpa(right)),
            (Value::Bool(left), Value::Bool(right)) => Some(left.eq(right)),
            (Value::String(Some(left)), Value::String(Some(right))) => {
                Some(Bool::from_bool(left == right))
            }
            (Value::Null, Value::Null) => Some(Bool::from_bool(true)),
            (Value::Int(_), Value::Float(_))
            | (Value::Float(_), Value::Int(_))
            | (Value::Int(_), Value::String(_))
            | (Value::String(_), Value::Int(_))
            | (Value::Float(_), Value::String(_))
            | (Value::String(_), Value::Float(_))
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
            (Value::Int(left), Value::Float(right)) => Some(if or_equal {
                left.to_real().le(&right.to_real())
            } else {
                left.to_real().lt(&right.to_real())
            }),
            (Value::Float(left), Value::Int(right)) => Some(if or_equal {
                left.to_real().le(&right.to_real())
            } else {
                left.to_real().lt(&right.to_real())
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
            (Value::Int(left), Value::Float(right)) => Some(if or_equal {
                left.to_real().ge(&right.to_real())
            } else {
                left.to_real().gt(&right.to_real())
            }),
            (Value::Float(left), Value::Int(right)) => Some(if or_equal {
                left.to_real().ge(&right.to_real())
            } else {
                left.to_real().gt(&right.to_real())
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
            _ => Value::Unknown,
        }
    }

    fn check_return_contract(&mut self, value: Option<&Value>, span: Span) {
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
                    "return-contract",
                    Message::new(
                        "E0031:return-contract",
                        "The function can return without a value but declares return type {type}.",
                    )
                    .arg("type", hint_name(hint)),
                    span,
                );
            }
            return;
        };

        if matches!(value, Value::Unknown) {
            return;
        }

        if hint_accepts_value(value, hint, self.strict_types) == Some(false) {
            self.push_message(
                "return-contract",
                Message::new(
                    "E0031:return-contract",
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
            if doc_type != "void" && doc_type != "never" && !contracts::is_nullable(doc_type) {
                self.push_message(
                    "return-contract",
                    Message::new(
                        "E0031:return-contract",
                        "The function can return without a value but documents return type {type}.",
                    )
                    .arg("type", doc_type),
                    span,
                );
            }
            return;
        };

        if matches!(value, Value::Unknown) {
            return;
        }

        if contracts::accepts_value(value, doc_type, true) == Some(false) {
            self.push_message(
                "return-contract",
                Message::new(
                    "E0031:return-contract",
                    "Returned {value} does not satisfy the documented return type {type}.",
                )
                .arg("value", value_type_name(value))
                .arg("type", doc_type),
                span,
            );
        }
    }

    fn check_intdiv(&mut self, left: &Value, right: &Value, span: Span, path: &Bool) {
        if !matches!(right, Value::Int(_)) {
            return;
        }
        self.check_nonzero(
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
        value: &Value,
        path: &Bool,
        span: Span,
        id: &'static str,
        template: &'static str,
    ) {
        let Some(zero) = value.zero_condition() else {
            return;
        };

        self.check_condition(&zero, path, span, id, template, Some(value));
    }

    fn check_assertion(&mut self, condition: &Bool, path: &Bool, span: Span) {
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
            SatResult::Unsat => {
                self.push_finding("assertion", "This assertion can never hold.", span, "false");
            }
            SatResult::Unknown => {
                if self.settings.unknown_policy == "report" {
                    self.push_finding(
                        "unverified",
                        "Could not verify this assertion: the solver returned unknown.",
                        span,
                        "?",
                    );
                }
            }
            SatResult::Sat => {}
        }
    }

    fn check_condition(
        &mut self,
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
                self.push_finding(id, template, span, &rendered);
            }
            SatResult::Unknown => {
                if self.settings.unknown_policy == "report" {
                    self.push_finding(
                        "unverified",
                        "Could not verify this property: the solver returned unknown.",
                        span,
                        "?",
                    );
                }
            }
            SatResult::Unsat => {}
        }
    }

    fn push_finding(&mut self, id: &'static str, template: &'static str, span: Span, value: &str) {
        let message = Message::new(format!("E0031:{id}"), template).arg("value", value);
        self.push_message(id, message, span);
    }

    fn push_message(&mut self, id: &'static str, message: Message, span: Span) {
        if self.settings.max_findings_per_function > 0
            && self.findings.len().saturating_sub(self.finding_limit_start)
                >= self.settings.max_findings_per_function
        {
            return;
        }

        let key = (span.start.offset, span.end.offset, id);
        if !self.seen.insert(key) {
            return;
        }

        self.findings.push(Finding { message, span });
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

fn builtin_call_value(name: Option<&str>, arguments: &[Value]) -> Option<Value> {
    let name = name?;
    match name {
        "strlen" => match arguments.first()? {
            Value::String(Some(value)) => Some(Value::Int(Int::from_u64(value.len() as u64))),
            _ => None,
        },
        "count" | "sizeof" => match arguments.first()? {
            Value::Array(Some(keys)) => Some(Value::Int(Int::from_u64(keys.len() as u64))),
            _ => None,
        },
        "array_key_exists" => {
            let key = array_key(arguments.first()?)?;
            match arguments.get(1)? {
                Value::Array(Some(keys)) => Some(Value::Bool(Bool::from_bool(keys.contains(&key)))),
                Value::Array(None) => None,
                _ => None,
            }
        }
        "is_int" | "is_integer" | "is_long" => Some(Value::Bool(Bool::from_bool(matches!(
            arguments.first()?,
            Value::Int(_)
        )))),
        "is_float" | "is_double" => Some(Value::Bool(Bool::from_bool(matches!(
            arguments.first()?,
            Value::Float(_)
        )))),
        "is_string" => Some(Value::Bool(Bool::from_bool(matches!(
            arguments.first()?,
            Value::String(_)
        )))),
        "is_bool" => Some(Value::Bool(Bool::from_bool(matches!(
            arguments.first()?,
            Value::Bool(_)
        )))),
        "is_array" => Some(Value::Bool(Bool::from_bool(matches!(
            arguments.first()?,
            Value::Array(_)
        )))),
        "is_null" => Some(Value::Bool(Bool::from_bool(matches!(
            arguments.first()?,
            Value::Null
        )))),
        _ => None,
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
        if doc_type == "string" || doc_type == "non-empty-string" || doc_type == "numeric-string" {
            return Value::String(None);
        }
        if doc_type == "bool" || doc_type == "boolean" || doc_type == "true" || doc_type == "false"
        {
            return Value::Bool(Bool::new_const(format!("{function}::{parameter}")));
        }
        if doc_type == "array"
            || doc_type == "iterable"
            || doc_type == "non-empty-array"
            || doc_type == "non-empty-list"
            || doc_type.starts_with("array<")
            || doc_type.starts_with("list<")
        {
            return Value::Array(None);
        }
        if doc_type == "null" {
            return Value::Null;
        }
        return Value::Unknown;
    }

    match hint {
        Some(Hint::Integer(_)) | Some(Hint::Mixed(_)) | None => {
            Value::Int(Int::new_const(format!("{function}::{parameter}")))
        }
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
) -> Option<Value> {
    if let Some(value) = summary.return_value.as_ref() {
        return Some(constant_to_value(value));
    }
    if !positional {
        return None;
    }
    summary
        .return_parameter
        .and_then(|index| arguments.get(index))
        .cloned()
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

fn path_may_be_true(path: &Bool) -> bool {
    !matches!(path.as_bool(), Some(false))
}

fn array_key(value: &Value) -> Option<ArrayKey> {
    match value {
        Value::Int(value) => value.as_i64().map(ArrayKey::Int),
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
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Null => "null",
        Value::Unknown => "unknown",
    }
}

fn hint_name(hint: &Hint<'_>) -> String {
    hint.to_string()
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
        Hint::True(_) | Hint::False(_) => match value {
            Value::Bool(value) => value.as_bool(),
            _ => Some(false),
        },
        Hint::Array(_) => Some(matches!(value, Value::Array(_))),
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
            Value::String(None) => None,
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
            Value::String(None) => None,
            _ => Some(false),
        },
        Hint::String(_) => match value {
            Value::String(_) => Some(true),
            Value::Int(_) | Value::Float(_) | Value::Bool(_) => Some(!strict),
            _ => Some(false),
        },
        Hint::Bool(_) => match value {
            Value::Bool(_) => Some(true),
            Value::Int(_) | Value::Float(_) | Value::String(_) => Some(!strict),
            _ => Some(false),
        },
        Hint::Void(_) | Hint::Never(_) => Some(false),
        Hint::Iterable(_) => Some(matches!(value, Value::Array(_))),
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
                    if let Expression::Literal(Literal::Integer(integer)) =
                        item.value.unparenthesized()
                    {
                        if integer.value == Some(1) {
                            return true;
                        }
                    }
                }
            }
            Statement::Namespace(namespace) => {
                if statements_declare_strict_types(namespace.statements().as_slice()) {
                    return true;
                }
            }
            _ => {}
        }
    }

    false
}
