use std::collections::{BTreeSet, HashMap};

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;

use crate::file::File;
use crate::rules::ast_support::{
    call_arguments, direct_variable_name, function_name, literal_string, method_name,
};

pub type Origins = BTreeSet<String>;

/// A call graph edge: caller function name -> callee function name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallGraph {
    /// caller -> set of callees
    pub edges: HashMap<String, BTreeSet<String>>,
    /// reverse: callee -> set of callers
    pub reverse: HashMap<String, BTreeSet<String>>,
    /// Functions that are defined but never called (potential entry points).
    pub entry_points: BTreeSet<String>,
    /// callee -> parameter indexes that received request-derived arguments.
    pub request_args: HashMap<String, BTreeSet<usize>>,
}

impl CallGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            reverse: HashMap::new(),
            entry_points: BTreeSet::new(),
            request_args: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, caller: &str, callee: &str) {
        self.edges.entry(caller.to_string()).or_default().insert(callee.to_string());
        self.reverse.entry(callee.to_string()).or_default().insert(caller.to_string());
    }

    pub fn add_request_arg(&mut self, callee: &str, index: usize) {
        self.request_args
            .entry(callee.to_string())
            .or_default()
            .insert(index);
    }

    pub fn callees(&self, caller: &str) -> Option<&BTreeSet<String>> {
        self.edges.get(caller)
    }

    pub fn callers(&self, callee: &str) -> Option<&BTreeSet<String>> {
        self.reverse.get(callee)
    }

    pub fn all_callers(&self) -> &HashMap<String, BTreeSet<String>> {
        &self.reverse
    }
}

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

/// The kind of a taint origin. Used to distinguish request-controlled data
/// (potentially exploitable) from parameter-derived data (a lead only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    #[default]
    Unknown,
    /// Data comes from a request source: $_GET, $_POST, $_COOKIE, $_FILES,
    /// $_SERVER, request object methods, php://input, filter_input(), etc.
    Request,
    /// Data comes from a function parameter whose origin is unknown.
    Parameter,
    /// Data is created inside the function (e.g. a local file read).
    Internal,
}

/// Filter for which source kinds should be reported.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum SourceFilter {
    /// Only report findings whose taint originates from a request source.
    #[default]
    Request,
    /// Report all tainted findings, including parameter-derived leads.
    All,
}

/// Structured information about a taint sink's source, for clean output.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SinkSourceInfo {
    /// Whether at least one origin is request-rooted.
    pub request_rooted: bool,
    /// The dominant source kind (Request > Parameter > Internal).
    pub source_kind: SourceKind,
    /// All origin strings (clean, no prefixes: e.g. `$_GET['id']`, `$value`).
    pub origins: Vec<String>,
    /// File where the tainted data originates (if known).
    pub source_file: Option<String>,
    /// Line where the tainted data originates (if known).
    pub source_line: Option<usize>,
    /// Name of the function containing the sink.
    pub enclosing_function: Option<String>,
}

/// A dangerous operation that the taint pass understands.
///
/// Every security rule that reasons about attacker-controlled data should query
/// one of these kinds instead of guessing from the shape of an expression. The
/// sink registry below decides which argument of a call feeds which kind, so a
/// rule only has to find the call and ask whether tainted data reaches it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SinkKind {
    Eval,
    AssertString,
    Unserialize,
    Include,
    Extract,
    VariableVariable,
    Sql,
    Html,
    Shell,
    Header,
    Mail,
    Ldap,
    XPath,
    Nosql,
    Xml,
    FilePath,
    Ssrf,
    Log,
    /// A tainted callable, class name or method receiver. Covers variable
    /// functions, dynamic instantiation and dynamic method calls.
    Callable,
    /// A tainted reflection member name or argument.
    Reflection,
    /// A tainted template source passed to a template engine.
    Template,
}

/// One source-to-sink flow discovered in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaintSink {
    pub kind: SinkKind,
    pub span: Span,
    pub call_span: Span,
    pub origins: Origins,
    pub via: Option<String>,
    /// True if any origin is request-rooted.
    pub request_rooted: bool,
    /// The dominant source kind for this sink.
    pub source_kind: SourceKind,
    /// Name of the function containing this sink (for interprocedural filtering).
    pub enclosing_function: Option<String>,
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

    /// Return structured source information for a sink, if found.
    pub fn sink_source_info(
        &self,
        kind: SinkKind,
        span: Span,
        file: &File<'_>,
    ) -> Option<SinkSourceInfo> {
        self.find(kind, span).map(|sink| {
            let mut displayed: BTreeSet<String> = sink
                .origins
                .iter()
                .map(|origin| base_origin(origin).to_string())
                .collect();
            displayed.retain(|origin| !origin.is_empty());
            let origins: Vec<String> = displayed.into_iter().collect();
            let source_kind = sink.source_kind;
            let source_line = file.line_number(sink.span.start.offset);
            let source_file = Some(file.path.to_string_lossy().to_string());
            SinkSourceInfo {
                request_rooted: sink.request_rooted,
                source_kind,
                origins,
                source_file,
                source_line: Some(source_line),
                enclosing_function: sink.enclosing_function.clone(),
            }
        })
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
    /// Parameter indexes whose taint reaches a sink inside the callee, with the sink's span.
    pub param_sinks: Vec<(usize, SinkKind, Span)>,
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
    /// Origins that have been validated by a regex guard (e.g. preg_match)
    /// in the current branch — sinks using these origins are skipped.
    sanitized_origins: BTreeSet<String>,
}

impl State {
    fn merge(&mut self, other: &State) {
        for (name, origins) in &other.env {
            self.env
                .entry(name.clone())
                .or_default()
                .extend(origins.iter().cloned());
        }
        let return_taint_clone = self.return_taint.clone();
        self.return_taint.extend(return_taint_clone.iter().cloned());
        // An origin is sanitized only if it is sanitized in ALL branches
        // (conservative: intersection)
        self.sanitized_origins = self
            .sanitized_origins
            .intersection(&other.sanitized_origins)
            .cloned()
            .collect();
    }
}

struct Analyzer<'summaries, 'arena> {
    summaries: &'summaries HashMap<String, TaintSummary>,
    sinks: Vec<TaintSink>,
    symbolic: bool,
    depth: usize,
    source_filter: SourceFilter,
    /// Call graph being built during analysis.
    call_graph: CallGraph,
    /// Current function name being analyzed (for call graph edges).
    current_function: Option<String>,
    _marker: std::marker::PhantomData<&'arena ()>,
}

impl<'summaries, 'arena> Analyzer<'summaries, 'arena> {
    fn new(
        summaries: &'summaries HashMap<String, TaintSummary>,
        symbolic: bool,
        source_filter: SourceFilter,
    ) -> Self {
        Self {
            summaries,
            sinks: Vec::new(),
            symbolic,
            depth: 0,
            source_filter,
            call_graph: CallGraph::new(),
            current_function: None,
            _marker: Default::default(),
        }
    }

    fn analyze_function(
        &mut self,
        current_func: Option<&str>,
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
        let prev_func = self.current_function.clone();
        self.current_function = current_func.map(|s| s.to_string());
        self.analyze_statements(body, &mut state);
        self.current_function = prev_func;
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
                    let origins = self.eval(value, state);
                    if !origins.is_empty() {
                        self.record(
                            SinkKind::Html,
                            value.span(),
                            value.span(),
                            origins,
                            None,
                            &state.sanitized_origins,
                        );
                    }
                }
            }
            Statement::EchoTag(statement) => {
                for value in statement.values.iter() {
                    let origins = self.eval(value, state);
                    if !origins.is_empty() {
                        self.record(
                            SinkKind::Html,
                            value.span(),
                            value.span(),
                            origins,
                            None,
                            &state.sanitized_origins,
                        );
                    }
                }
            }
            Statement::If(statement) => {
                self.eval(statement.condition, state);
                // Check if the if-condition is a regex guard (preg_match) — if so,
                // mark the checked variable as sanitized in the then-branch.
                if let Some(guard_var) = is_regex_guard_call(statement.condition.unparenthesized()) {
                    let mut then_state = clone_state(state);
                    // Look up the variable's taint origins and mark them as sanitized
                    if let Some(origins) = state.env.get(&guard_var) {
                        for origin in origins {
                            then_state.sanitized_origins.insert(origin.clone());
                        }
                    }
                    self.analyze_if_body(&statement.body, &mut then_state);
                } else {
                    self.analyze_if_body(&statement.body, state);
                }
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
                // Backticks are shell execution, not string interpolation.
                if matches!(string, CompositeString::ShellExecute(_)) && !origins.is_empty() {
                    self.record(
                        SinkKind::Shell,
                        string.span(),
                        string.span(),
                        origins.clone(),
                        None,
                        &state.sanitized_origins,
                    );
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
                // `new $class(...)` is a callable sink when the class name is
                // attacker-controlled.
                let class_origins = self.eval(instantiation.class, state);
                if !class_origins.is_empty() {
                    self.record(
                        SinkKind::Callable,
                        instantiation.span(),
                        instantiation.span(),
                        class_origins,
                        None,
                        &state.sanitized_origins,
                    );
                }
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
            let key = literal_string(access.index)?;
            if is_request_server_key(&key) {
                return Some(single_origin(format!("$_SERVER[{key}]")));
            }
            return None;
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
                if is_superglobal(&name) {
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
                        &state.sanitized_origins,
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
                        &state.sanitized_origins,
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
        let lhs_name = direct_variable_name(assignment.lhs).unwrap_or_default().to_string();
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

        let call_span = call_span(call);

        // A tainted callee is a sink of its own: variable functions, dynamic
        // method receivers, dynamic method names and dynamic class names let
        // an attacker choose what code runs. This must happen before the name
        // lookup because a dynamic method has no static name.
        let mut callee_taint = match call {
            Call::Function(call) => self.eval(call.function, state),
            Call::Method(call) => self.eval(call.object, state),
            Call::NullSafeMethod(call) => self.eval(call.object, state),
            Call::StaticMethod(call) => self.eval(call.class, state),
        };
        match call {
            Call::Method(call) => {
                callee_taint.extend(self.selector_taint(&call.method, state));
            }
            Call::NullSafeMethod(call) => {
                callee_taint.extend(self.selector_taint(&call.method, state));
            }
            Call::StaticMethod(call) => {
                callee_taint.extend(self.selector_taint(&call.method, state));
            }
            Call::Function(_) => {}
        }
        if !callee_taint.is_empty() {
            self.record(
                SinkKind::Callable,
                call_span,
                call_span,
                callee_taint,
                None,
                &state.sanitized_origins,
            );
        }

        let Some(name) = call_name(call) else {
            return union(&argument_taints);
        };
        let is_method = !matches!(call, Call::Function(_));

        // Remember when a request-derived value is passed to a function, so
        // cross-file analysis can recognise the callee's sinks as request-rooted.
        for (index, taint) in argument_taints.iter().enumerate() {
            if taint.iter().any(|origin| is_request_origin(origin)) {
                self.call_graph.add_request_arg(&name, index);
            }
        }

        // Record call graph edge: current function -> callee
        if let Some(caller) = &self.current_function {
            self.call_graph.add_edge(caller, &name);
        }

        match call {
            Call::Function(call) => {
                if let Some(origins) = self.function_source(&name, &arguments) {
                    self.record_argument_sinks(
                        &name,
                        is_method,
                        call.span(),
                        &arguments,
                        &argument_taints,
                        &state.sanitized_origins,
                    );
                    return origins;
                }
            }
            Call::Method(call) => {
                if is_request_method(&name) && is_request_object(call.object) {
                    self.record_argument_sinks(
                        &name,
                        is_method,
                        call.span(),
                        &arguments,
                        &argument_taints,
                        &state.sanitized_origins,
                    );
                    return single_origin(format!("request->{name}()"));
                }
            }
            Call::NullSafeMethod(call) => {
                if is_request_method(&name) && is_request_object(call.object) {
                    self.record_argument_sinks(
                        &name,
                        is_method,
                        call.span(),
                        &arguments,
                        &argument_taints,
                        &state.sanitized_origins,
                    );
                    return single_origin(format!("request->{name}()"));
                }
            }
            Call::StaticMethod(_) => {}
        }

        self.record_argument_sinks(
            &name,
            is_method,
            call_span,
            &arguments,
            &argument_taints,
            &state.sanitized_origins,
        );

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
            if let Some(param_sinks) = self.summary(&name).map(|s| s.param_sinks.clone()) {
                for (index, kind, span) in &param_sinks {
                    if let Some(taint) = argument_taints.get(*index) {
                        if !taint.is_empty() {
                            // Record at the sink's span (inside the callee body) so it
                            // merges with the sink already found during callee analysis,
                            // updating request_rooted based on the caller's argument taint.
                            self.record(
                                *kind,
                                *span,
                                *span,
                                taint.clone(),
                                Some(name.clone()),
                                &state.sanitized_origins,
                            );
                        }
                    }
                }
            }
            origins
        } else if let Some(scope) = sanitizer_scope(&name) {
            // Keep the origin so it is still tainted for sinks the sanitizer
            // does not cover, but mark it as neutralized for its own context.
            mark_sanitized(&union(&argument_taints), scope)
        } else {
            union(&argument_taints)
        }
    }

    /// Taint carried by a dynamic method selector such as `$object->{$name}()`.
    fn selector_taint(
        &mut self,
        selector: &'arena ClassLikeMemberSelector<'arena>,
        state: &mut State,
    ) -> Origins {
        match selector {
            ClassLikeMemberSelector::Variable(variable) => self.eval_variable(variable, state),
            ClassLikeMemberSelector::Expression(selector) => self.eval(selector.expression, state),
            ClassLikeMemberSelector::Identifier(_) | ClassLikeMemberSelector::Missing(_) => {
                Origins::new()
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record_argument_sinks(
        &mut self,
        name: &str,
        is_method: bool,
        call_span: Span,
        arguments: &[&'arena Expression<'arena>],
        argument_taints: &[Origins],
        sanitized_origins: &BTreeSet<String>,
    ) {
        // `assert()` evaluates its argument as code when given a string. The
        // taint engine does not track types, so any tainted argument is
        // reported; an untainted boolean expression stays clean.
        if !is_method && name == "assert" {
            if let Some(taint) = argument_taints.first() {
                if !taint.is_empty() {
                    self.record(
                        SinkKind::AssertString,
                        arguments[0].span(),
                        call_span,
                        taint.clone(),
                        None,
                        sanitized_origins,
                    );
                }
            }
        }

        for spec in call_sink_args(name, is_method) {
            let mut origins = Origins::new();
            let mut anchor: Option<Span> = None;
            if spec.all {
                for (index, taint) in argument_taints.iter().enumerate() {
                    if !taint.is_empty() {
                        origins.extend(taint.iter().cloned());
                        if anchor.is_none() {
                            anchor = arguments.get(index).map(|argument| argument.span());
                        }
                    }
                }
            } else {
                for index in spec.indexes {
                    if let Some(taint) = argument_taints.get(*index) {
                        if !taint.is_empty() {
                            origins.extend(taint.iter().cloned());
                            if anchor.is_none() {
                                anchor = arguments.get(*index).map(|argument| argument.span());
                            }
                        }
                    }
                }
            }
            if origins.is_empty() {
                continue;
            }
            self.record(
                spec.kind,
                anchor.unwrap_or(call_span),
                call_span,
                origins,
                None,
                sanitized_origins,
            );
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
                        &state.sanitized_origins,
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
                        &state.sanitized_origins,
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
                        &state.sanitized_origins,
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
                        &state.sanitized_origins,
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
                        &state.sanitized_origins,
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
                let origins = self.eval(construct.value, state);
                if !origins.is_empty() {
                    self.record(
                        SinkKind::Html,
                        construct.value.span(),
                        construct.value.span(),
                        origins,
                        None,
                        &state.sanitized_origins,
                    );
                }
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
            "filter_input" => Some(single_origin(String::from("filter_input()"))),
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
        sanitized_origins: &BTreeSet<String>,
    ) {
        // Drop origins that a sanitizer has already neutralized for this sink's
        // context. An HTML escape still leaves SQL/command data dangerous, so
        // the marker is scoped rather than removing the origin outright.
        let origins: Origins = origins
            .into_iter()
            .filter(|origin| !origin_sanitized_for(origin, kind))
            .collect();
        if origins.is_empty() {
            return;
        }

        let already = self.sinks.iter().any(|sink| {
            sink.kind == kind && same_span(sink.span, span)
        });
        if already {
            // Merge origins instead of skipping - this handles interprocedural
            // taint where the same sink is reached from multiple call sites
            if let Some(existing) = self.sinks.iter_mut().find(|sink| {
                sink.kind == kind && same_span(sink.span, span)
            }) {
                existing.origins.extend(origins);
                existing.via = existing.via.clone().or(via.clone());
                existing.request_rooted = existing.origins.iter().any(|o| is_request_origin(o));
                existing.source_kind = if existing.request_rooted {
                    SourceKind::Request
                } else if existing
                    .origins
                    .iter()
                    .any(|o| base_origin(o).starts_with('$'))
                {
                    SourceKind::Parameter
                } else {
                    SourceKind::Internal
                };
            }
            return;
        }
        let request_rooted = origins.iter().any(|o| is_request_origin(o));
        let source_kind = if request_rooted {
            SourceKind::Request
        } else if origins.iter().any(|o| base_origin(o).starts_with('$')) {
            SourceKind::Parameter
        } else {
            SourceKind::Internal
        };

        if is_guard_suppressed(&origins, sanitized_origins) {
            return;
        }

        self.sinks.push(TaintSink {
            kind,
            span,
            call_span,
            origins,
            via,
            request_rooted,
            source_kind,
            enclosing_function: self.current_function.clone(),
        });
    }
}

fn clone_state(state: &State) -> State {
    State {
        env: state.env.clone(),
        return_taint: state.return_taint.clone(),
        sanitized_origins: state.sanitized_origins.clone(),
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

/// A sink fed by one or more arguments of a call.
///
/// `all` marks functions where every argument can reach the sink (`printf`,
/// `mail` and friends); otherwise `indexes` lists the arguments that matter.
#[derive(Clone, Copy)]
struct SinkArg {
    kind: SinkKind,
    all: bool,
    indexes: &'static [usize],
}

const fn sink(kind: SinkKind, indexes: &'static [usize]) -> SinkArg {
    SinkArg {
        kind,
        all: false,
        indexes,
    }
}

const fn sink_all(kind: SinkKind) -> SinkArg {
    SinkArg {
        kind,
        all: true,
        indexes: &[],
    }
}

/// Sinks reached through a plain function call.
fn function_sink_args(name: &str) -> Vec<SinkArg> {
    match name {
        // SQL. `mysqli_query($link, $query)` and `sqlite_query($link, $query)`
        // put the query second; the older helpers put it first.
        "mysqli_query" | "mysqli_real_query" | "mysqli_multi_query" | "sqlite_query"
        | "sqlite_exec" => vec![sink(SinkKind::Sql, &[1])],
        "mysql_query" => vec![sink(SinkKind::Sql, &[0])],
        "pg_query" | "pg_exec" | "pg_send_query" => vec![sink(SinkKind::Sql, &[0, 1])],

        // Command execution.
        "system" | "exec" | "shell_exec" | "passthru" | "popen" | "proc_open"
        | "pcntl_exec" => vec![sink(SinkKind::Shell, &[0])],

        // Deserialization.
        "unserialize" | "wddx_deserialize" | "session_decode" => {
            vec![sink(SinkKind::Unserialize, &[0])]
        }

        "extract" => vec![sink(SinkKind::Extract, &[0])],

        // Direct output.
        "printf" | "vprintf" => vec![sink_all(SinkKind::Html)],

        // Response headers and cookies.
        "header" | "header_remove" | "setcookie" | "setrawcookie" => {
            vec![sink_all(SinkKind::Header)]
        }

        // Mail headers/recipients.
        "mail" | "mb_send_mail" => vec![sink_all(SinkKind::Mail)],

        // LDAP search filters are the third argument.
        "ldap_search" | "ldap_list" | "ldap_read" => {
            vec![sink(SinkKind::Ldap, &[2])]
        }

        // XML/XPath parsing.
        "simplexml_load_string" | "simplexml_load_file" => {
            vec![sink(SinkKind::Xml, &[0]), sink(SinkKind::FilePath, &[0])]
        }

        // Filesystem. These are also potential SSRF vectors when the path is a
        // URL, so record both kinds and let each rule ask for its own.
        "file_get_contents" | "file" | "readfile" | "fopen" | "file_put_contents" | "unlink"
        | "file_exists" | "is_file" | "is_dir" | "scandir" | "opendir" => {
            vec![sink(SinkKind::FilePath, &[0]), sink(SinkKind::Ssrf, &[0])]
        }
        "curl_init" | "fsockopen" => vec![sink(SinkKind::Ssrf, &[0])],
        "curl_exec" => vec![sink(SinkKind::Ssrf, &[0])],

        // Logs.
        "error_log" => vec![sink(SinkKind::Log, &[0, 2])],
        "syslog" => vec![sink(SinkKind::Log, &[1])],

        // NoSQL drivers.
        "mongo_insert" | "mongo_update" | "mongo_find" => vec![sink_all(SinkKind::Nosql)],

        // Template engines exposed as functions. The rule treats the whole
        // call as the sink, so every argument is considered.
        "str_replace" | "preg_replace" => vec![sink_all(SinkKind::Template)],

        // Upload destinations.
        "move_uploaded_file" => vec![sink(SinkKind::FilePath, &[1])],

        _ => vec![],
    }
}

/// Sinks reached through `$object->method()`, `$object?->method()` or
/// `Class::method()`. The class name is not available here (see `call_name`),
/// so this table only includes method names whose sink is unambiguous.
fn method_sink_args(name: &str) -> Vec<SinkArg> {
    match name {
        "query" | "exec" | "real_query" | "multi_query" => vec![sink(SinkKind::Sql, &[0, 1])],
        "load" => vec![sink(SinkKind::Xml, &[0]), sink(SinkKind::FilePath, &[0])],
        "loadxml" => vec![sink(SinkKind::Xml, &[0])],
        "xpath" => vec![sink(SinkKind::XPath, &[0])],
        "search" => vec![sink(SinkKind::Ldap, &[1])],
        // Reflection.
        "invoke" | "invokeargs" | "setaccessible" | "getvalue" | "setvalue" => {
            vec![sink_all(SinkKind::Reflection)]
        }
        // Template engines.
        "render" | "renderblock" | "createtemplate" => vec![sink_all(SinkKind::Template)],
        // NoSQL manager.
        "createclient" => vec![sink_all(SinkKind::Nosql)],
        // NoSQL collection operations.
        "insertone" | "insertmany" | "updateone" | "updatemany" | "replaceone" | "deleteone"
        | "deletemany" | "findone" | "find" | "aggregate" | "countdocuments" | "distinct" => {
            vec![sink_all(SinkKind::Nosql)]
        }
        _ => vec![],
    }
}

fn call_sink_args(name: &str, is_method: bool) -> Vec<SinkArg> {
    if is_method {
        method_sink_args(name)
    } else {
        function_sink_args(name)
    }
}

/// Marker wrapped around an origin that has passed through a sanitizer. The
/// scope says which sink kind the sanitizer neutralizes; the underlying origin
/// is kept so the value still counts as tainted for other sinks.
const SANITIZE_MARK: char = '\u{1}';
const SANITIZE_SEP: char = '\u{2}';

fn mark_sanitized(origins: &Origins, scope: &str) -> Origins {
    origins
        .iter()
        .map(|origin| {
            if origin_has_scope(origin, scope) {
                origin.clone()
            } else {
                format!("{SANITIZE_MARK}{scope}{SANITIZE_SEP}{origin}")
            }
        })
        .collect()
}

/// The sanitizer scope for a function name, if it is a sanitizer.
///
/// `all` means the output is safe in any context (numeric casts, hashes).
/// The other scopes only neutralize their own sink kind, so escaping a value
/// for HTML does not hide a SQL injection.
fn sanitizer_scope(name: &str) -> Option<&'static str> {
    match name {
        "intval" | "floatval" | "boolval" | "abs" | "strlen" | "count" | "sizeof" | "ord"
        | "bin2hex" | "dechex" | "md5" | "sha1" | "hash" | "hash_hmac" | "crc32" => {
            Some("all")
        }
        "htmlspecialchars" | "htmlentities" | "strip_tags" | "esc_html" | "esc_attr" => {
            Some("html")
        }
        "escapeshellarg" | "escapeshellcmd" => Some("shell"),
        "addslashes" | "mysqli_real_escape_string" | "pg_escape_string" | "sqlite_escape_string" => {
            Some("sql")
        }
        "basename" => Some("path"),
        _ => None,
    }
}

fn split_sanitized(origin: &str) -> Option<(&str, &str)> {
    let rest = origin.strip_prefix(SANITIZE_MARK)?;
    rest.split_once(SANITIZE_SEP)
}

/// Strip every sanitizer marker and return the underlying origin.
fn base_origin(origin: &str) -> &str {
    let mut current = origin;
    while let Some((_, inner)) = split_sanitized(current) {
        current = inner;
    }
    current
}

fn origin_has_scope(origin: &str, scope: &str) -> bool {
    let mut current = origin;
    while let Some((found, inner)) = split_sanitized(current) {
        if found == scope {
            return true;
        }
        current = inner;
    }
    false
}

fn origin_sanitized_for(origin: &str, kind: SinkKind) -> bool {
    let mut current = origin;
    while let Some((scope, inner)) = split_sanitized(current) {
        if scope == "all" || scope_neutralizes(scope, kind) {
            return true;
        }
        current = inner;
    }
    false
}

fn scope_neutralizes(scope: &str, kind: SinkKind) -> bool {
    match scope {
        "html" => kind == SinkKind::Html,
        "shell" => kind == SinkKind::Shell,
        "sql" => kind == SinkKind::Sql,
        "path" => kind == SinkKind::FilePath,
        _ => false,
    }
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

/// The span of any call form.
fn call_span(call: &Call<'_>) -> Span {
    match call {
        Call::Function(call) => call.span(),
        Call::Method(call) => call.span(),
        Call::NullSafeMethod(call) => call.span(),
        Call::StaticMethod(call) => call.span(),
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

/// Returns true if an origin string represents request-controlled data.
fn is_request_origin(origin: &str) -> bool {
    let o = base_origin(origin).trim();
    // Superglobals
    o.starts_with("$_GET")
        || o.starts_with("$_POST")
        || o.starts_with("$_COOKIE")
        || o.starts_with("$_FILES")
        || o.starts_with("$_REQUEST")
        || o.starts_with("$_SERVER")
        // Request helper functions
        || o == "filter_input()"
        || o == "getallheaders()"
        || o == "apache_request_headers()"
        || o == "php://input"
        // Request object methods (e.g. $request->input())
        || o.ends_with("()")
            && ["input", "query", "post", "get", "all", "json", "file", "cookie", "header", "param"]
                .iter()
                .any(|m| o.contains(m))
}

/// Check if an expression is a `preg_match()` or `preg_match_all()` guard call
/// and return the variable name being validated. Returns `None` if not a guard.
fn is_regex_guard_call(expression: &Expression<'_>) -> Option<String> {
    let Expression::Call(Call::Function(call)) = expression.unparenthesized() else {
        return None;
    };
    let Some(name) = function_name(call.function) else {
        return None;
    };
    if name != "preg_match" && name != "preg_match_all" {
        return None;
    }
    let arguments: Vec<&Expression<'_>> = call.argument_list.arguments.iter().map(|arg| arg.value()).collect();
    // preg_match(pattern, $subject) — the subject is the 2nd argument
    let subject = arguments.get(1)?;
    match subject.unparenthesized() {
        Expression::Variable(Variable::Direct(variable)) => {
            let name = variable_name(variable.name);
            Some(name)
        }
        _ => None,
    }
}

/// Check whether a sink should be suppressed because all its tainted origins
/// are covered by regex guard variables (e.g. preg_match validated them).
fn is_guard_suppressed(origins: &Origins, sanitized_origins: &BTreeSet<String>) -> bool {
    if origins.is_empty() {
        return false;
    }
    let result = origins.iter().all(|o| sanitized_origins.contains(o));
    result
}

pub(crate) fn analyze_file<'arena>(
    file: &File<'arena>,
    summaries: &HashMap<String, TaintSummary>,
    source_filter: SourceFilter,
) -> (FileTaint, CallGraph, HashMap<String, TaintSummary>) {
    let Some(program) = file.ast else {
        return (FileTaint::default(), CallGraph::new(), HashMap::new());
    };

    // Phase 1: Analyze all function bodies first so their summaries are available
    // when analyzing top-level code that calls them.
    let functions = collect_function_bodies(program.statements.as_slice());
    let (phase1_graph, function_summaries, phase1_sinks) = {
        let mut analyzer = Analyzer::new(summaries, true, source_filter);
        let mut function_summaries = HashMap::new();
        let mut phase1_sinks = Vec::new();

        for function in &functions {
            let function_name = function.names.first().map(|s| s.as_str());
            let (return_taint, mut function_sinks) = analyzer.analyze_function(
                function_name,
                &function.params,
                function.body,
            );

            // Compute the summary from the sinks before moving them out.
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
            for sink in &function_sinks {
                for origin in &sink.origins {
                    if let Some(index) = parameter_index(origin, &function.params) {
                        let entry = (index, sink.kind, sink.span);
                        if !param_sinks.contains(&entry) {
                            param_sinks.push(entry);
                        }
                    }
                }
            }

            phase1_sinks.append(&mut function_sinks);

            if let Some(name) = function_name {
                function_summaries.insert(
                    name.to_string().to_ascii_lowercase(),
                    TaintSummary {
                        params: function.params.clone(),
                        return_from,
                        returns_source,
                        param_sinks,
                    },
                );
            }
        }

        (
            std::mem::take(&mut analyzer.call_graph),
            function_summaries,
            phase1_sinks,
        )
    };

    // Phase 2: Analyze top-level code with summaries from both other files and
    // this file, seeded with the sinks found inside function bodies so that
    // interprocedural propagation can merge into them.
    let mut all_summaries = summaries.clone();
    all_summaries.extend(
        function_summaries
            .iter()
            .map(|(name, summary)| (name.clone(), summary.clone())),
    );

    let mut analyzer = Analyzer::new(&all_summaries, true, source_filter);
    analyzer.sinks = phase1_sinks;
    analyzer.call_graph = phase1_graph;

    let mut state = State::default();
    analyzer.analyze_statements(program.statements.as_slice(), &mut state);

    let sinks = std::mem::take(&mut analyzer.sinks);
    let call_graph = analyzer.call_graph;
    (FileTaint::new(sinks), call_graph, function_summaries)
}

pub(crate) fn summarize_file<'arena>(
    file: &File<'arena>,
    _source_filter: SourceFilter,
) -> Vec<(String, TaintSummary)> {
    let Some(program) = file.ast else {
        return Vec::new();
    };

    let functions = collect_function_bodies(program.statements.as_slice());
    let mut summaries: HashMap<String, TaintSummary> = HashMap::new();

    // Iterate to a fixpoint so helpers that call other helpers propagate
    // taint transitively (bounded to keep pathological chains cheap).
    let max_passes = functions.len().clamp(1, 8);
    for _ in 0..max_passes {
        let mut next = HashMap::new();
        for function in &functions {
            let summary = summarize_function(function, &summaries);
            for name in &function.names {
                next.entry(name.clone().to_ascii_lowercase()).or_insert_with(|| summary.clone());
            }
        }
        if next == summaries {
            break;
        }
        summaries = next;
    }

    let mut entries = Vec::new();
    for function in &functions {
        for name in &function.names {
            if let Some(summary) = summaries.get(&name.to_ascii_lowercase()) {
                entries.push((name.to_ascii_lowercase(), summary.clone()));
            }
        }
    }
    entries
}

fn summarize_function(
    function: &FunctionDef<'_>,
    summaries: &HashMap<String, TaintSummary>,
) -> TaintSummary {
    let function_name = function.names.first().map(|s| s.as_str());
    let mut analyzer = Analyzer::new(summaries, true, SourceFilter::All);
    let (return_taint, sinks) = analyzer.analyze_function(function_name, &function.params, function.body);

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
                let entry = (index, sink.kind, sink.span);
                if !param_sinks.contains(&entry) {
                    param_sinks.push(entry);
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
    let origin = base_origin(origin);
    let origin = origin.strip_prefix('$').unwrap_or(origin);
    params.iter().position(|param| param.strip_prefix('$').unwrap_or(param) == origin)
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
