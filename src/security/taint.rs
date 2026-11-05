use std::collections::{BTreeSet, HashMap};

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;

use crate::file::File;
use crate::rules::ast_support::{
    call_arguments, direct_variable_name, function_name, literal_string, method_name,
};

pub type Origins = BTreeSet<String>;

/// Taint-aware reporting mode for a security rule.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaintMode {
    /// Keep the existing heuristic behaviour and ignore taint.
    Off,
    /// Keep every finding, but add the source when taint is proven.
    #[default]
    Annotate,
    /// Only report findings reached by proven tainted data.
    Only,
}

/// A dangerous operation that the taint pass understands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SinkKind {
    Eval,
    AssertString,
    Unserialize,
    Include,
    Extract,
    VariableVariable,
}

/// One source-to-sink flow discovered in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaintSink {
    pub kind: SinkKind,
    pub span: Span,
    pub call_span: Span,
    pub origins: Origins,
    pub via: Option<String>,
}

/// Taint findings for a single file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileTaint {
    sinks: Vec<TaintSink>,
}

impl FileTaint {
    pub(crate) fn new(sinks: Vec<TaintSink>) -> Self {
        Self { sinks }
    }

    /// Find a sink matching either the dangerous expression span or the
    /// enclosing call/construct span.
    pub fn find(&self, kind: SinkKind, span: Span) -> Option<&TaintSink> {
        self.sinks
            .iter()
            .find(|sink| sink.kind == kind && same_span(sink.span, span))
            .or_else(|| {
                self.sinks
                    .iter()
                    .find(|sink| sink.kind == kind && same_span(sink.call_span, span))
            })
    }

    pub fn origin(&self, kind: SinkKind, span: Span) -> Option<&str> {
        self.find(kind, span)
            .and_then(|sink| sink.origins.iter().next().map(String::as_str))
    }
}

fn same_span(left: Span, right: Span) -> bool {
    left.start.offset == right.start.offset && left.end.offset == right.end.offset
}

/// A compact model of how a function or method moves taint.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TaintSummary {
    pub params: Vec<String>,
    /// Parameter indexes whose taint reaches the return value.
    pub return_from: Vec<usize>,
    /// The return value is tainted by a source created inside the callee.
    pub returns_source: bool,
    /// Parameter indexes whose taint reaches a sink inside the callee.
    pub param_sinks: Vec<(usize, SinkKind)>,
}

struct FunctionDef<'arena> {
    names: Vec<String>,
    params: Vec<String>,
    body: &'arena [Statement<'arena>],
}

#[derive(Default)]
struct State {
    env: HashMap<String, Origins>,
    return_taint: Origins,
}

impl State {
    fn merge(&mut self, other: &State) {
        for (name, origins) in &other.env {
            self.env
                .entry(name.clone())
                .or_default()
                .extend(origins.iter().cloned());
        }
        self.return_taint.extend(other.return_taint.iter().cloned());
    }
}

struct Analyzer<'summaries, 'arena> {
    summaries: &'summaries HashMap<String, TaintSummary>,
    sinks: Vec<TaintSink>,
    symbolic: bool,
    depth: usize,
    _marker: std::marker::PhantomData<&'arena ()>,
}

impl<'summaries, 'arena> Analyzer<'summaries, 'arena> {
    fn new(summaries: &'summaries HashMap<String, TaintSummary>, symbolic: bool) -> Self {
        Self {
            summaries,
            sinks: Vec::new(),
            symbolic,
            depth: 0,
            _marker: Default::default(),
        }
    }

    fn analyze_function(
        &mut self,
        params: &[String],
        body: &'arena [Statement<'arena>],
    ) -> (Origins, Vec<TaintSink>) {
        let mut state = State::default();
        if self.symbolic {
            for param in params {
                let name = param.trim_start_matches('$').to_string();
                let mut origins = Origins::new();
                origins.insert(format!("${name}"));
                state.env.insert(name, origins);
            }
        }
        self.analyze_statements(body, &mut state);
        (state.return_taint, std::mem::take(&mut self.sinks))
    }

    fn analyze_statements(&mut self, statements: &'arena [Statement<'arena>], state: &mut State) {
        for statement in statements {
            self.analyze_statement(statement, state);
        }
    }

    fn analyze_statement(&mut self, statement: &'arena Statement<'arena>, state: &mut State) {
        match statement {
            Statement::Expression(statement) => {
                self.eval(statement.expression, state);
            }
            Statement::Return(statement) => {
                if let Some(value) = statement.value {
                    let origins = self.eval(value, state);
                    state.return_taint.extend(origins);
                }
            }
            Statement::Echo(statement) => {
                for value in statement.values.iter() {
                    self.eval(value, state);
                }
            }
            Statement::EchoTag(statement) => {
                for value in statement.values.iter() {
                    self.eval(value, state);
                }
            }
            Statement::If(statement) => {
                self.eval(statement.condition, state);
                self.analyze_if_body(&statement.body, state);
            }
            Statement::While(statement) => {
                self.eval(statement.condition, state);
                self.analyze_statements(statement.body.statements(), state);
            }
            Statement::DoWhile(statement) => {
                self.analyze_statement(statement.statement, state);
                self.eval(statement.condition, state);
            }
            Statement::For(statement) => {
                for expression in statement.initializations.iter() {
                    self.eval(expression, state);
                }
                for expression in statement.conditions.iter() {
                    self.eval(expression, state);
                }
                self.analyze_statements(statement.body.statements(), state);
                for expression in statement.increments.iter() {
                    self.eval(expression, state);
                }
            }
            Statement::Foreach(statement) => {
                let iterable = self.eval(statement.expression, state);
                if let Some(key) = statement.target.key() {
                    self.assign_pattern(key, &iterable, state);
                }
                self.assign_pattern(statement.target.value(), &iterable, state);
                self.analyze_statements(statement.body.statements(), state);
            }
            Statement::Switch(statement) => {
                self.eval(statement.expression, state);
                for case in statement.body.cases().iter() {
                    if let Some(expression) = case.expression() {
                        self.eval(expression, state);
                    }
                    self.analyze_statements(case.statements(), state);
                }
            }
            Statement::Try(statement) => {
                self.analyze_statements(statement.block.statements.as_slice(), state);
                for catch in statement.catch_clauses.iter() {
                    self.analyze_statements(catch.block.statements.as_slice(), state);
                }
                if let Some(finally) = &statement.finally_clause {
                    self.analyze_statements(finally.block.statements.as_slice(), state);
                }
            }
            Statement::Namespace(namespace) => {
                self.analyze_statements(namespace.statements().as_slice(), state);
            }
            Statement::Block(block) => {
                self.analyze_statements(block.statements.as_slice(), state);
            }
            Statement::Declare(declare) => match &declare.body {
                DeclareBody::Statement(statement) => self.analyze_statement(statement, state),
                DeclareBody::ColonDelimited(body) => {
                    self.analyze_statements(body.statements.as_slice(), state);
                }
            },
            Statement::Unset(unset) => {
                for value in unset.values.iter() {
                    if let Some(name) = direct_variable_name(value) {
                        state.env.remove(name);
                    }
                }
            }
            Statement::Static(static_) => {
                for item in static_.items.iter() {
                    if let StaticItem::Concrete(item) = item {
                        let origins = self.eval(item.value, state);
                        state.env.insert(
                            String::from_utf8_lossy(item.variable.name).to_string(),
                            origins,
                        );
                    }
                }
            }
            // Functions, classes, interfaces, traits, and enums are handled by
            // the project index and separate per-body passes.
            _ => {}
        }
    }

    fn analyze_if_body(&mut self, body: &'arena IfBody<'arena>, state: &mut State) {
        match body {
            IfBody::Statement(body) => {
                let mut then_state = clone_state(state);
                self.analyze_statement(body.statement, &mut then_state);
                for clause in body.else_if_clauses.iter() {
                    self.eval(clause.condition, &mut then_state);
                    self.analyze_statement(clause.statement, &mut then_state);
                }
                if let Some(clause) = &body.else_clause {
                    self.analyze_statement(clause.statement, &mut then_state);
                }
                state.merge(&then_state);
            }
            IfBody::ColonDelimited(body) => {
                let mut then_state = clone_state(state);
                self.analyze_statements(body.statements.as_slice(), &mut then_state);
                for clause in body.else_if_clauses.iter() {
                    self.eval(clause.condition, &mut then_state);
                    self.analyze_statements(clause.statements.as_slice(), &mut then_state);
                }
                if let Some(clause) = &body.else_clause {
                    self.analyze_statements(clause.statements.as_slice(), &mut then_state);
                }
                state.merge(&then_state);
            }
        }
    }

    fn eval(&mut self, expression: &'arena Expression<'arena>, state: &mut State) -> Origins {
        if self.depth >= 48 {
            return Origins::new();
        }
        self.depth += 1;
        let result = self.eval_inner(expression, state);
        self.depth -= 1;
        result
    }

    fn eval_inner(&mut self, expression: &'arena Expression<'arena>, state: &mut State) -> Origins {
        if let Some(source) = self.source_origins(expression, state) {
            return source;
        }

        match expression.unparenthesized() {
            Expression::Literal(_)
            | Expression::MagicConstant(_)
            | Expression::ConstantAccess(_)
            | Expression::Identifier(_)
            | Expression::Parent(_)
            | Expression::Self_(_)
            | Expression::Static(_)
            | Expression::Error(_) => Origins::new(),
            Expression::Variable(variable) => self.eval_variable(variable, state),
            Expression::ArrayAccess(access) => self.eval(access.array, state),
            Expression::ArrayAppend(append) => self.eval(append.array, state),
            Expression::Assignment(assignment) => self.eval_assignment(assignment, state),
            Expression::Binary(binary) => {
                let mut origins = self.eval(binary.lhs, state);
                origins.extend(self.eval(binary.rhs, state));
                origins
            }
            Expression::UnaryPrefix(unary) => {
                if matches!(
                    unary.operator,
                    UnaryPrefixOperator::BoolCast(_, _)
                        | UnaryPrefixOperator::BooleanCast(_, _)
                        | UnaryPrefixOperator::DoubleCast(_, _)
                        | UnaryPrefixOperator::RealCast(_, _)
                        | UnaryPrefixOperator::FloatCast(_, _)
                        | UnaryPrefixOperator::IntCast(_, _)
                        | UnaryPrefixOperator::IntegerCast(_, _)
                ) {
                    self.eval(unary.operand, state);
                    Origins::new()
                } else {
                    self.eval(unary.operand, state)
                }
            }
            Expression::UnaryPostfix(unary) => self.eval(unary.operand, state),
            Expression::Parenthesized(parenthesized) => self.eval(parenthesized.expression, state),
            Expression::Conditional(conditional) => {
                let mut origins = self.eval(conditional.condition, state);
                if let Some(then) = conditional.then {
                    origins.extend(self.eval(then, state));
                }
                origins.extend(self.eval(conditional.r#else, state));
                origins
            }
            Expression::CompositeString(string) => {
                let mut origins = Origins::new();
                for part in string.parts().iter() {
                    match part {
                        StringPart::Literal(_) => {}
                        StringPart::Expression(expression) => {
                            origins.extend(self.eval(expression, state));
                        }
                        StringPart::BracedExpression(braced) => {
                            origins.extend(self.eval(braced.expression, state));
                        }
                    }
                }
                origins
            }
            Expression::Array(array) => self.eval_array_elements(&array.elements, state),
            Expression::LegacyArray(array) => self.eval_array_elements(&array.elements, state),
            Expression::List(list) => self.eval_array_elements(&list.elements, state),
            Expression::Call(call) => self.eval_call(call, state),
            Expression::Construct(construct) => self.eval_construct(construct, state),
            Expression::Access(access) => match access {
                Access::Property(access) => self.eval(access.object, state),
                Access::NullSafeProperty(access) => self.eval(access.object, state),
                Access::StaticProperty(_) | Access::ClassConstant(_) => Origins::new(),
            },
            Expression::Instantiation(instantiation) => {
                let mut origins = Origins::new();
                if let Some(arguments) = &instantiation.argument_list {
                    for argument in arguments.arguments.iter() {
                        origins.extend(self.eval(argument.value(), state));
                    }
                }
                origins
            }
            Expression::Closure(closure) => {
                let mut closure_state = clone_state(state);
                self.analyze_statements(closure.body.statements.as_slice(), &mut closure_state);
                Origins::new()
            }
            Expression::ArrowFunction(function) => {
                let mut closure_state = clone_state(state);
                self.eval(function.expression, &mut closure_state);
                Origins::new()
            }
            Expression::Match(match_) => {
                let mut origins = self.eval(match_.expression, state);
                for arm in match_.arms.iter() {
                    if let MatchArm::Expression(arm) = arm {
                        for condition in arm.conditions.iter() {
                            origins.extend(self.eval(condition, state));
                        }
                    }
                    origins.extend(self.eval(arm.expression(), state));
                }
                origins
            }
            Expression::Yield(yield_) => {
                match yield_ {
                    Yield::Value(value) => {
                        if let Some(value) = value.value {
                            self.eval(value, state);
                        }
                    }
                    Yield::Pair(pair) => {
                        self.eval(pair.key, state);
                        self.eval(pair.value, state);
                    }
                    Yield::From(from) => {
                        self.eval(from.iterator, state);
                    }
                }
                Origins::new()
            }
            Expression::Throw(throw) => {
                self.eval(throw.exception, state);
                Origins::new()
            }
            Expression::Clone(clone) => self.eval(clone.object, state),
            Expression::Pipe(pipe) => {
                let mut origins = self.eval(pipe.input, state);
                origins.extend(self.eval(pipe.callable, state));
                origins
            }
            Expression::AnonymousClass(class) => {
                for member in class.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(body) = &method.body {
                            let mut method_state = clone_state(state);
                            self.analyze_statements(body.statements.as_slice(), &mut method_state);
                        }
                    }
                }
                Origins::new()
            }
            Expression::PartialApplication(_) => Origins::new(),
            _ => Origins::new(),
        }
    }

    fn source_access(&mut self, access: &mago_syntax::cst::ArrayAccess<'arena>) -> Option<Origins> {
        let base = access.array.unparenthesized();
        if let Expression::ArrayAccess(inner) = base {
            return self.source_access(inner);
        }
        let Expression::Variable(Variable::Direct(variable)) = base else {
            return None;
        };
        let name = variable_name(variable.name);
        if name == "_SERVER" {
            let Some(key) = literal_string(access.index) else {
                // A dynamic key may reach a request field; stay conservative.
                return Some(single_origin(String::from("$_SERVER")));
            };
            if is_request_server_key(&key) {
                return Some(single_origin(format!("$_SERVER[{key}]")));
            }
            // Literal keys such as SCRIPT_FILENAME or APP_RUNTIME are server
            // configuration, not request data. Returning an empty result stops
            // eval_inner from falling back to the bare superglobal source.
            return Some(Origins::new());
        }
        if !is_superglobal(&name) {
            return None;
        }
        let origin = literal_string(access.index)
            .map(|key| format!("${name}['{key}']"))
            .unwrap_or_else(|| format!("${name}[...]"));
        Some(single_origin(origin))
    }

    fn source_origins(
        &mut self,
        expression: &'arena Expression<'arena>,
        state: &mut State,
    ) -> Option<Origins> {
        match expression.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                let name = variable_name(variable.name);
                if is_superglobal(&name) || name == "_SERVER" {
                    Some(single_origin(format!("${name}")))
                } else {
                    None
                }
            }
            Expression::ArrayAccess(access) => self.source_access(access),
            _ => {
                let _ = state;
                None
            }
        }
    }

    fn eval_variable(&mut self, variable: &'arena Variable<'arena>, state: &mut State) -> Origins {
        match variable {
            Variable::Direct(variable) => {
                let name = variable_name(variable.name);
                state.env.get(&name).cloned().unwrap_or_default()
            }
            Variable::Indirect(variable) => {
                let origins = self.eval(variable.expression, state);
                if !origins.is_empty() {
                    self.record(
                        SinkKind::VariableVariable,
                        variable.span(),
                        variable.span(),
                        origins,
                        None,
                    );
                }
                Origins::new()
            }
            Variable::Nested(variable) => {
                let origins = self.variable_name_taint(variable.variable, state);
                if !origins.is_empty() {
                    self.record(
                        SinkKind::VariableVariable,
                        variable.span(),
                        variable.span(),
                        origins,
                        None,
                    );
                }
                Origins::new()
            }
        }
    }

    fn variable_name_taint(
        &mut self,
        variable: &'arena Variable<'arena>,
        state: &mut State,
    ) -> Origins {
        match variable {
            Variable::Direct(variable) => {
                let name = variable_name(variable.name);
                state.env.get(&name).cloned().unwrap_or_default()
            }
            Variable::Indirect(variable) => self.eval(variable.expression, state),
            Variable::Nested(variable) => self.variable_name_taint(variable.variable, state),
        }
    }

    fn eval_assignment(
        &mut self,
        assignment: &'arena Assignment<'arena>,
        state: &mut State,
    ) -> Origins {
        let _ = self.eval(assignment.lhs, state);
        let rhs = self.eval(assignment.rhs, state);

        if let Some(name) = direct_variable_name(assignment.lhs) {
            match assignment.operator {
                AssignmentOperator::Assign(_) => {
                    state.env.insert(name.to_string(), rhs.clone());
                }
                _ => {
                    state
                        .env
                        .entry(name.to_string())
                        .or_default()
                        .extend(rhs.iter().cloned());
                }
            }
        } else {
            self.assign_pattern(assignment.lhs, &rhs, state);
        }

        rhs
    }

    fn assign_pattern(
        &mut self,
        pattern: &'arena Expression<'arena>,
        origins: &Origins,
        state: &mut State,
    ) {
        match pattern.unparenthesized() {
            Expression::Variable(Variable::Direct(variable)) => {
                state
                    .env
                    .insert(variable_name(variable.name), origins.clone());
            }
            Expression::Array(array) => {
                for element in array.elements.iter() {
                    if let Some(value) = element.get_value() {
                        self.assign_pattern(value, origins, state);
                    }
                }
            }
            Expression::LegacyArray(array) => {
                for element in array.elements.iter() {
                    if let Some(value) = element.get_value() {
                        self.assign_pattern(value, origins, state);
                    }
                }
            }
            Expression::List(list) => {
                for element in list.elements.iter() {
                    if let Some(value) = element.get_value() {
                        self.assign_pattern(value, origins, state);
                    }
                }
            }
            _ => {}
        }
    }

    fn eval_array_elements(
        &mut self,
        elements: &'arena TokenSeparatedSequence<'arena, ArrayElement<'arena>>,
        state: &mut State,
    ) -> Origins {
        let mut origins = Origins::new();
        for element in elements.iter() {
            match element {
                ArrayElement::KeyValue(element) => {
                    self.eval(element.key, state);
                    origins.extend(self.eval(element.value, state));
                }
                ArrayElement::Value(element) => origins.extend(self.eval(element.value, state)),
                ArrayElement::Variadic(element) => origins.extend(self.eval(element.value, state)),
                ArrayElement::Missing(_) => {}
            }
        }
        origins
    }

    fn eval_call(&mut self, call: &'arena Call<'arena>, state: &mut State) -> Origins {
        let arguments = call_arguments(call);
        let argument_taints = arguments
            .iter()
            .map(|argument| self.eval(argument, state))
            .collect::<Vec<_>>();

        let Some(name) = call_name(call) else {
            return union(&argument_taints);
        };

        match call {
            Call::Function(call) => {
                if let Some(origins) = self.function_source(&name, &arguments) {
                    self.record_argument_sinks(&name, call.span(), &arguments, &argument_taints);
                    return origins;
                }
            }
            Call::Method(call) => {
                if is_request_method(&name) && is_request_object(call.object) {
                    self.record_argument_sinks(&name, call.span(), &arguments, &argument_taints);
                    return single_origin(format!("request->{name}()"));
                }
            }
            Call::NullSafeMethod(call) => {
                if is_request_method(&name) && is_request_object(call.object) {
                    self.record_argument_sinks(&name, call.span(), &arguments, &argument_taints);
                    return single_origin(format!("request->{name}()"));
                }
            }
            Call::StaticMethod(_) => {}
        }

        self.record_argument_sinks(&name, call.span(), &arguments, &argument_taints);

        if let Some(summary) = self.summary(&name) {
            let mut origins = Origins::new();
            for index in &summary.return_from {
                if let Some(taint) = argument_taints.get(*index) {
                    origins.extend(taint.iter().cloned());
                }
            }
            if summary.returns_source {
                origins.insert(format!("{name}()"));
            }
            let param_sinks = summary.param_sinks.clone();
            for (index, kind) in &param_sinks {
                if let Some(taint) = argument_taints.get(*index) {
                    if !taint.is_empty() {
                        self.record(
                            *kind,
                            call.span(),
                            call.span(),
                            taint.clone(),
                            Some(name.clone()),
                        );
                    }
                }
            }
            origins
        } else if is_sanitizer(&name) {
            Origins::new()
        } else {
            union(&argument_taints)
        }
    }

    fn record_argument_sinks(
        &mut self,
        name: &str,
        call_span: Span,
        arguments: &[&'arena Expression<'arena>],
        argument_taints: &[Origins],
    ) {
        match name {
            "assert" => {
                if let Some(taint) = argument_taints.first() {
                    if !taint.is_empty() && arguments.first().is_some_and(|arg| is_code_string(arg))
                    {
                        self.record(
                            SinkKind::AssertString,
                            arguments[0].span(),
                            call_span,
                            taint.clone(),
                            None,
                        );
                    }
                }
            }
            "unserialize" => {
                if let Some(taint) = argument_taints.first() {
                    if !taint.is_empty() {
                        self.record(
                            SinkKind::Unserialize,
                            arguments[0].span(),
                            call_span,
                            taint.clone(),
                            None,
                        );
                    }
                }
            }
            "extract" => {
                if let Some(taint) = argument_taints.first() {
                    if !taint.is_empty() {
                        self.record(
                            SinkKind::Extract,
                            arguments[0].span(),
                            call_span,
                            taint.clone(),
                            None,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn eval_construct(
        &mut self,
        construct: &'arena Construct<'arena>,
        state: &mut State,
    ) -> Origins {
        match construct {
            Construct::Eval(construct) => {
                let origins = self.eval(construct.value, state);
                if !origins.is_empty() {
                    self.record(
                        SinkKind::Eval,
                        construct.value.span(),
                        construct.span(),
                        origins,
                        None,
                    );
                }
            }
            Construct::Include(construct) => {
                let origins = self.eval(construct.value, state);
                if !origins.is_empty() {
                    self.record(
                        SinkKind::Include,
                        construct.value.span(),
                        construct.span(),
                        origins,
                        None,
                    );
                }
            }
            Construct::IncludeOnce(construct) => {
                let origins = self.eval(construct.value, state);
                if !origins.is_empty() {
                    self.record(
                        SinkKind::Include,
                        construct.value.span(),
                        construct.span(),
                        origins,
                        None,
                    );
                }
            }
            Construct::Require(construct) => {
                let origins = self.eval(construct.value, state);
                if !origins.is_empty() {
                    self.record(
                        SinkKind::Include,
                        construct.value.span(),
                        construct.span(),
                        origins,
                        None,
                    );
                }
            }
            Construct::RequireOnce(construct) => {
                let origins = self.eval(construct.value, state);
                if !origins.is_empty() {
                    self.record(
                        SinkKind::Include,
                        construct.value.span(),
                        construct.span(),
                        origins,
                        None,
                    );
                }
            }
            Construct::Isset(construct) => {
                for value in construct.values.iter() {
                    self.eval(value, state);
                }
            }
            Construct::Empty(construct) => {
                self.eval(construct.value, state);
            }
            Construct::Print(construct) => {
                self.eval(construct.value, state);
            }
            Construct::Exit(construct) => {
                if let Some(arguments) = &construct.arguments {
                    for argument in arguments.arguments.iter() {
                        self.eval(argument.value(), state);
                    }
                }
            }
            Construct::Die(construct) => {
                if let Some(arguments) = &construct.arguments {
                    for argument in arguments.arguments.iter() {
                        self.eval(argument.value(), state);
                    }
                }
            }
        }
        Origins::new()
    }

    fn function_source(
        &mut self,
        name: &str,
        arguments: &[&'arena Expression<'arena>],
    ) -> Option<Origins> {
        match name {
            "getallheaders" | "apache_request_headers" => Some(single_origin(format!("{name}()"))),
            "filter_input" | "filter_input_array" => Some(single_origin(format!("{name}()"))),
            "file_get_contents"
                if arguments
                    .first()
                    .and_then(|argument| literal_string(argument))
                    .is_some_and(|path| path.contains("php://input")) =>
            {
                Some(single_origin(String::from("php://input")))
            }
            _ => None,
        }
    }

    fn summary(&self, name: &str) -> Option<&TaintSummary> {
        self.summaries.get(name)
    }

    fn record(
        &mut self,
        kind: SinkKind,
        span: Span,
        call_span: Span,
        origins: Origins,
        via: Option<String>,
    ) {
        let already = self.sinks.iter().any(|sink| {
            sink.kind == kind && same_span(sink.span, span) && same_span(sink.call_span, call_span)
        });
        if already {
            return;
        }
        self.sinks.push(TaintSink {
            kind,
            span,
            call_span,
            origins,
            via,
        });
    }
}

fn clone_state(state: &State) -> State {
    State {
        env: state.env.clone(),
        return_taint: state.return_taint.clone(),
    }
}

fn union(values: &[Origins]) -> Origins {
    let mut origins = Origins::new();
    for value in values {
        origins.extend(value.iter().cloned());
    }
    origins
}

fn single_origin(origin: String) -> Origins {
    let mut origins = Origins::new();
    origins.insert(origin);
    origins
}

fn variable_name(name: &[u8]) -> String {
    let name = String::from_utf8_lossy(name);
    name.strip_prefix('$').unwrap_or(&name).to_string()
}

fn call_name(call: &Call<'_>) -> Option<String> {
    match call {
        Call::Function(call) => function_name(call.function),
        Call::Method(call) => method_name(&call.method),
        Call::NullSafeMethod(call) => method_name(&call.method),
        Call::StaticMethod(call) => method_name(&call.method),
    }
}

fn is_superglobal(name: &str) -> bool {
    matches!(name, "_GET" | "_POST" | "_REQUEST" | "_COOKIE" | "_FILES")
}

fn is_request_server_key(key: &str) -> bool {
    key.starts_with("HTTP_")
        || key.starts_with("REQUEST_")
        || key.starts_with("PHP_AUTH_")
        || matches!(
            key,
            "QUERY_STRING"
                | "PATH_INFO"
                | "CONTENT_TYPE"
                | "CONTENT_LENGTH"
                | "REMOTE_ADDR"
                | "REMOTE_HOST"
                | "REMOTE_PORT"
                | "REQUEST_METHOD"
                | "SCRIPT_NAME"
                | "SERVER_NAME"
                | "SERVER_PORT"
                | "HTTPS"
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

fn is_request_object(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Variable(Variable::Direct(variable)) => variable_name(variable.name)
            .to_ascii_lowercase()
            .contains("request"),
        Expression::Access(Access::Property(access)) => {
            method_name(&access.property).is_some_and(|name| name.contains("request"))
        }
        Expression::Access(Access::NullSafeProperty(access)) => {
            method_name(&access.property).is_some_and(|name| name.contains("request"))
        }
        _ => false,
    }
}

fn is_code_string(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::Literal(Literal::String(_)) => true,
        Expression::Binary(binary) => matches!(binary.operator, BinaryOperator::StringConcat(_)),
        _ => false,
    }
}

fn is_sanitizer(name: &str) -> bool {
    matches!(
        name,
        "intval"
            | "floatval"
            | "boolval"
            | "abs"
            | "strlen"
            | "count"
            | "sizeof"
            | "ord"
            | "bin2hex"
            | "dechex"
            | "md5"
            | "sha1"
            | "hash"
            | "hash_hmac"
            | "crc32"
    )
}

pub(crate) fn analyze_file<'arena>(
    file: &File<'arena>,
    summaries: &HashMap<String, TaintSummary>,
) -> FileTaint {
    let Some(program) = file.ast else {
        return FileTaint::default();
    };

    let mut sinks = Vec::new();

    let mut top_level = Analyzer::new(summaries, false);
    let mut state = State::default();
    top_level.analyze_statements(program.statements.as_slice(), &mut state);
    sinks.append(&mut top_level.sinks);

    let functions = collect_function_bodies(program.statements.as_slice());
    for function in functions {
        let mut analyzer = Analyzer::new(summaries, true);
        let (_, mut function_sinks) = analyzer.analyze_function(&function.params, function.body);
        sinks.append(&mut function_sinks);
    }

    FileTaint::new(sinks)
}

pub(crate) fn summarize_file<'arena>(file: &File<'arena>) -> Vec<(String, TaintSummary)> {
    let Some(program) = file.ast else {
        return Vec::new();
    };

    let functions = collect_function_bodies(program.statements.as_slice());
    let mut summaries: HashMap<String, TaintSummary> = HashMap::new();

    // Two passes let helpers call each other with a useful local summary.
    for _ in 0..2 {
        let mut next = HashMap::new();
        for function in &functions {
            let summary = summarize_function(function, &summaries);
            for name in &function.names {
                next.entry(name.clone()).or_insert_with(|| summary.clone());
            }
        }
        summaries = next;
    }

    let mut entries = Vec::new();
    for function in &functions {
        for name in &function.names {
            if let Some(summary) = summaries.get(name) {
                entries.push((name.clone(), summary.clone()));
            }
        }
    }
    entries
}

fn summarize_function(
    function: &FunctionDef<'_>,
    summaries: &HashMap<String, TaintSummary>,
) -> TaintSummary {
    let mut analyzer = Analyzer::new(summaries, true);
    let (return_taint, sinks) = analyzer.analyze_function(&function.params, function.body);

    let mut return_from = Vec::new();
    let mut returns_source = false;
    for origin in &return_taint {
        if let Some(index) = parameter_index(origin, &function.params) {
            if !return_from.contains(&index) {
                return_from.push(index);
            }
        } else {
            returns_source = true;
        }
    }

    let mut param_sinks = Vec::new();
    for sink in sinks {
        for origin in &sink.origins {
            if let Some(index) = parameter_index(origin, &function.params) {
                if !param_sinks.contains(&(index, sink.kind)) {
                    param_sinks.push((index, sink.kind));
                }
            }
        }
    }

    TaintSummary {
        params: function.params.clone(),
        return_from,
        returns_source,
        param_sinks,
    }
}

fn parameter_index(origin: &str, params: &[String]) -> Option<usize> {
    let origin = origin.strip_prefix('$').unwrap_or(origin);
    params
        .iter()
        .position(|param| param.strip_prefix('$').unwrap_or(param) == origin)
}

fn collect_function_bodies<'arena>(
    statements: &'arena [Statement<'arena>],
) -> Vec<FunctionDef<'arena>> {
    let mut functions = Vec::new();
    collect_functions_from_statements(statements, &mut functions);
    functions
}

fn collect_functions_from_statements<'arena>(
    statements: &'arena [Statement<'arena>],
    functions: &mut Vec<FunctionDef<'arena>>,
) {
    for statement in statements {
        match statement {
            Statement::Namespace(namespace) => {
                collect_functions_from_statements(namespace.statements().as_slice(), functions);
            }
            Statement::Block(block) => {
                collect_functions_from_statements(block.statements.as_slice(), functions);
            }
            Statement::Declare(declare) => match &declare.body {
                DeclareBody::Statement(statement) => {
                    collect_functions_from_statements(std::slice::from_ref(statement), functions)
                }
                DeclareBody::ColonDelimited(body) => {
                    collect_functions_from_statements(body.statements.as_slice(), functions)
                }
            },
            Statement::Function(function) => {
                functions.push(FunctionDef {
                    names: vec![String::from_utf8_lossy(function.name.value).to_string()],
                    params: parameter_names(&function.parameter_list),
                    body: function.body.statements.as_slice(),
                });
                collect_functions_from_statements(function.body.statements.as_slice(), functions);
            }
            Statement::Class(class) => {
                collect_functions_from_members(
                    class.members.as_slice(),
                    Some(&String::from_utf8_lossy(class.name.value)),
                    functions,
                );
            }
            Statement::Interface(interface) => {
                collect_functions_from_members(
                    interface.members.as_slice(),
                    Some(&String::from_utf8_lossy(interface.name.value)),
                    functions,
                );
            }
            Statement::Trait(r#trait) => {
                collect_functions_from_members(
                    r#trait.members.as_slice(),
                    Some(&String::from_utf8_lossy(r#trait.name.value)),
                    functions,
                );
            }
            Statement::Enum(r#enum) => {
                collect_functions_from_members(
                    r#enum.members.as_slice(),
                    Some(&String::from_utf8_lossy(r#enum.name.value)),
                    functions,
                );
            }
            _ => {}
        }
    }
}

fn collect_functions_from_members<'arena>(
    members: &'arena [ClassLikeMember<'arena>],
    class: Option<&str>,
    functions: &mut Vec<FunctionDef<'arena>>,
) {
    for member in members {
        let ClassLikeMember::Method(method) = member else {
            continue;
        };
        let MethodBody::Concrete(body) = &method.body else {
            continue;
        };
        let name = String::from_utf8_lossy(method.name.value).to_string();
        let mut names = vec![name.clone()];
        if let Some(class) = class {
            names.push(format!("{class}::{name}"));
        }
        functions.push(FunctionDef {
            names,
            params: parameter_names(&method.parameter_list),
            body: body.statements.as_slice(),
        });
        collect_functions_from_statements(body.statements.as_slice(), functions);
    }
}

fn parameter_names(parameters: &FunctionLikeParameterList<'_>) -> Vec<String> {
    parameters
        .parameters
        .iter()
        .map(|parameter| String::from_utf8_lossy(parameter.variable.name).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::SecurityService;
    use mago_allocator::prelude::LocalArena;
    use std::path::PathBuf;

    fn analyze_pair(source: &str, sink: &str) -> FileTaint {
        let service = SecurityService::new();
        let arena = LocalArena::new();
        let source = File::new(&arena, PathBuf::from("source.php"), source.to_string());
        let sink = File::new(&arena, PathBuf::from("sink.php"), sink.to_string());

        service.begin_index();
        service.index_file(&source);
        service.index_file(&sink);
        service.finalize_index();

        service.analyze(&sink)
    }

    fn analyze_single(source: &str) -> FileTaint {
        let arena = LocalArena::new();
        let file = File::new(&arena, PathBuf::from("s.php"), source.to_string());
        analyze_file(&file, &HashMap::new())
    }

    #[test]
    fn treats_filter_input_array_as_a_source() {
        let taint = analyze_single("<?php eval(filter_input_array(INPUT_GET));");
        assert!(
            taint
                .sinks
                .iter()
                .any(|sink| sink.kind == SinkKind::Eval
                    && sink.origins.contains("filter_input_array()")),
            "got {:?}",
            taint.sinks
        );
    }

    #[test]
    fn treats_bare_server_superglobal_as_a_source() {
        let taint = analyze_single("<?php eval(implode($_SERVER));");
        assert!(
            taint
                .sinks
                .iter()
                .any(|sink| sink.kind == SinkKind::Eval && sink.origins.contains("$_SERVER")),
            "got {:?}",
            taint.sinks
        );
    }

    #[test]
    fn ignores_non_request_literal_server_keys() {
        let taint = analyze_single("<?php eval(base64_decode($_SERVER['SCRIPT_FILENAME']));");
        assert!(
            taint.sinks.iter().all(|sink| sink.kind != SinkKind::Eval),
            "got {:?}",
            taint.sinks
        );
    }

    #[test]
    fn keeps_dynamic_server_keys_tainted() {
        let taint = analyze_single("<?php eval($_SERVER[$key]);");
        assert!(
            taint
                .sinks
                .iter()
                .any(|sink| sink.kind == SinkKind::Eval && sink.origins.contains("$_SERVER")),
            "got {:?}",
            taint.sinks
        );
    }

    #[test]
    fn propagates_source_returning_function_across_files() {
        let taint = analyze_pair(
            "<?php function user_code() { return $_GET['code']; }",
            "<?php eval(user_code());",
        );

        assert!(
            taint
                .sinks
                .iter()
                .any(|sink| sink.kind == SinkKind::Eval && sink.origins.contains("user_code()")),
            "expected cross-file source flow, got {:?}",
            taint.sinks
        );
    }

    #[test]
    fn propagates_parameter_taint_into_callee_sink_across_files() {
        let taint = analyze_pair(
            "<?php function run_code($code) { eval($code); }",
            "<?php run_code($_GET['code']);",
        );

        assert!(
            taint
                .sinks
                .iter()
                .any(|sink| sink.kind == SinkKind::Eval && !sink.origins.is_empty()),
            "expected cross-file parameter flow, got {:?}",
            taint.sinks
        );
    }

    #[test]
    fn ignores_ambiguous_cross_file_function_names() {
        let service = SecurityService::new();
        let arena = LocalArena::new();
        let first = File::new(
            &arena,
            PathBuf::from("one.php"),
            "<?php function user_code() { return $_GET['code']; }".to_string(),
        );
        let second = File::new(
            &arena,
            PathBuf::from("two.php"),
            "<?php function user_code() { return 'safe'; }".to_string(),
        );
        let sink = File::new(
            &arena,
            PathBuf::from("three.php"),
            "<?php eval(user_code());".to_string(),
        );

        service.begin_index();
        service.index_file(&first);
        service.index_file(&second);
        service.index_file(&sink);
        service.finalize_index();

        let taint = service.analyze(&sink);
        assert!(
            taint.sinks.iter().all(|sink| sink.kind != SinkKind::Eval),
            "ambiguous names must not be trusted, got {:?}",
            taint.sinks
        );
    }
}
