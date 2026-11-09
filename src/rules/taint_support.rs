//! Shared plumbing for security rules backed by the taint engine.
//!
//! A migrated rule asks the engine whether a source reaches its sink instead of
//! guessing from the shape of the expression. The settings keep the old
//! behaviour available for projects that explicitly opt in, but the default is
//! `only`, so a plain variable is never reported as user input.

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;
use serde::{Deserialize, Serialize};

use crate::file::File;
use crate::results::{Message, TaintSource, Violation};
use crate::rules::ast_support::{
    call_arguments, call_name, call_span, is_method_call, walk_statement_expressions,
};
use crate::rules::Rule;
use crate::security::{FileTaint, SinkKind, TaintMode};

/// Per-rule taint settings, shared by every migrated security rule.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub taint_mode: TaintMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Injection rules are about source-to-sink flow. Reporting every
            // variable-shaped argument was the old behaviour and is the main
            // source of unexplainable findings.
            taint_mode: TaintMode::Only,
        }
    }
}

/// Whether a candidate should be reported.
///
/// `legacy` is the result of the old "does this expression look dynamic"
/// heuristic. It is only consulted when a project opts out of proven-flow mode,
/// which keeps existing configurations working without making the heuristic
/// the default.
pub(crate) fn should_report(
    settings: &Settings,
    taint: &FileTaint,
    kind: SinkKind,
    span: Span,
    legacy: bool,
) -> bool {
    match settings.taint_mode {
        TaintMode::Only => taint.find(kind, span).is_some(),
        TaintMode::Annotate | TaintMode::Off => legacy,
    }
}

/// Copy proven source information onto a violation.
pub(crate) fn annotate(
    settings: &Settings,
    violation: &mut Violation,
    taint: &FileTaint,
    kind: SinkKind,
    span: Span,
    file: &File<'_>,
) {
    if settings.taint_mode == TaintMode::Off {
        return;
    }

    let Some(info) = taint.sink_source_info(kind, span, file) else {
        return;
    };

    violation.taint_source = Some(TaintSource {
        request_rooted: info.request_rooted,
        source_kind: Some(format!("{:?}", info.source_kind).to_lowercase()),
        origins: info.origins,
        file: info.source_file,
        line: info.source_line,
        enclosing_function: info.enclosing_function,
    });
}

/// The old heuristic, kept only for `off` and `annotate` compatibility.
///
/// It intentionally answers "could this expression contain a variable at all",
/// not "is this expression attacker-controlled". New code must use the taint
/// engine instead.
pub(crate) fn looks_dynamic(expression: &Expression<'_>) -> bool {
    match expression.unparenthesized() {
        Expression::ArrayAccess(access) => {
            matches!(
                access.array.unparenthesized(),
                Expression::Variable(Variable::Direct(_))
            )
        }
        Expression::Variable(Variable::Direct(_)) => true,
        Expression::Binary(binary) if matches!(binary.operator, BinaryOperator::StringConcat(_)) => {
            looks_dynamic(binary.lhs) || looks_dynamic(binary.rhs)
        }
        Expression::CompositeString(string) => string
            .parts()
            .iter()
            .any(|part| matches!(part, StringPart::Expression(_) | StringPart::BracedExpression(_))),
        _ => false,
    }
}

/// Walk every call in a file and report the ones whose arguments reach `kind`.
///
/// `is_candidate` decides whether a call is this rule's sink, and `message`
/// builds the rule-specific wording. Keeping the traversal here means every
/// migrated rule handles every call form (function, method, null-safe, static)
/// and every argument shape identically.
pub(crate) fn scan_call_sinks(
    rule: &dyn Rule,
    settings: &Settings,
    file: &File<'_>,
    taint: &FileTaint,
    kind: SinkKind,
    is_candidate: impl Fn(&str, bool) -> bool,
    message: impl Fn(&str) -> Message,
) -> Option<Vec<Violation>> {
    let program = file.ast?;
    let mut violations = Vec::new();

    for statement in program.statements.iter() {
        walk_statement_expressions(statement, &mut |expression| {
            let Expression::Call(call) = expression else {
                return;
            };
            let Some(name) = call_name(call) else {
                return;
            };
            if !is_candidate(&name, is_method_call(call)) {
                return;
            }

            let arguments = call_arguments(call);
            if arguments.is_empty() {
                return;
            }

            let span = call_span(call);
            let legacy = arguments.iter().any(|argument| looks_dynamic(argument));
            if !should_report(settings, taint, kind, span, legacy) {
                return;
            }

            let mut violation = rule.new_violation(file, message(&name), span);
            annotate(settings, &mut violation, taint, kind, span, file);
            violations.push(violation);
        });
    }

    Some(violations)
}

/// Walk every call and report the ones whose callee (function name, class,
/// receiver or method selector) is tainted.
///
/// Unlike [`scan_call_sinks`], this does not require a static call name, so it
/// also covers dynamic method names such as `$object->{$name}()`.
pub(crate) fn scan_callable_sinks(
    rule: &dyn Rule,
    settings: &Settings,
    file: &File<'_>,
    taint: &FileTaint,
    is_candidate: impl Fn(&Call<'_>) -> bool,
    legacy: impl Fn(&Call<'_>) -> bool,
    message: impl Fn() -> Message,
) -> Option<Vec<Violation>> {
    let program = file.ast?;
    let mut violations = Vec::new();

    for statement in program.statements.iter() {
        walk_statement_expressions(statement, &mut |expression| {
            let Expression::Call(call) = expression else {
                return;
            };
            if !is_candidate(call) {
                return;
            }

            let span = call_span(call);
            if !should_report(settings, taint, SinkKind::Callable, span, legacy(call)) {
                return;
            }

            let mut violation = rule.new_violation(file, message(), span);
            annotate(settings, &mut violation, taint, SinkKind::Callable, span, file);
            violations.push(violation);
        });
    }

    Some(violations)
}

/// The source text name of an `include`/`require` construct.
pub(crate) fn construct_name(construct: &Construct<'_>) -> &'static str {
    match construct {
        Construct::Include(_) => "include",
        Construct::IncludeOnce(_) => "include_once",
        Construct::Require(_) => "require",
        Construct::RequireOnce(_) => "require_once",
        Construct::Eval(_) => "eval",
        _ => "construct",
    }
}

/// The expression a construct operates on, for the constructs this module
/// understands.
pub(crate) fn construct_value<'a>(construct: &'a Construct<'a>) -> Option<&'a Expression<'a>> {
    match construct {
        Construct::Include(inc) => Some(&inc.value),
        Construct::IncludeOnce(inc) => Some(&inc.value),
        Construct::Require(req) => Some(&req.value),
        Construct::RequireOnce(req) => Some(&req.value),
        Construct::Eval(eval) => Some(&eval.value),
        _ => None,
    }
}

/// Walk every `include`/`require` construct and report tainted paths.
pub(crate) fn scan_includes(
    rule: &dyn Rule,
    settings: &Settings,
    file: &File<'_>,
    taint: &FileTaint,
    message: impl Fn(&str) -> Message,
) -> Option<Vec<Violation>> {
    scan_constructs(
        rule,
        settings,
        file,
        taint,
        SinkKind::Include,
        message,
    )
}

/// Walk every `eval` construct and report tainted code strings.
pub(crate) fn scan_eval(
    rule: &dyn Rule,
    settings: &Settings,
    file: &File<'_>,
    taint: &FileTaint,
    message: impl Fn(&str) -> Message,
) -> Option<Vec<Violation>> {
    scan_constructs(rule, settings, file, taint, SinkKind::Eval, message)
}

fn scan_constructs(
    rule: &dyn Rule,
    settings: &Settings,
    file: &File<'_>,
    taint: &FileTaint,
    kind: SinkKind,
    message: impl Fn(&str) -> Message,
) -> Option<Vec<Violation>> {
    let program = file.ast?;
    let mut violations = Vec::new();

    for statement in program.statements.iter() {
        walk_statement_expressions(statement, &mut |expression| {
            let Expression::Construct(construct) = expression else {
                return;
            };
            let matches_kind = match (kind, construct) {
                (SinkKind::Include, Construct::Include(_))
                | (SinkKind::Include, Construct::IncludeOnce(_))
                | (SinkKind::Include, Construct::Require(_))
                | (SinkKind::Include, Construct::RequireOnce(_))
                | (SinkKind::Eval, Construct::Eval(_)) => true,
                _ => false,
            };
            if !matches_kind {
                return;
            }

            let Some(value) = construct_value(construct) else {
                return;
            };
            let span = value.span();
            let legacy = looks_dynamic(value);
            if !should_report(settings, taint, kind, span, legacy) {
                return;
            }

            let mut violation = rule.new_violation(file, message(construct_name(construct)), span);
            annotate(settings, &mut violation, taint, kind, span, file);
            violations.push(violation);
        });
    }

    Some(violations)
}

/// Walk every backtick shell execution and report tainted interpolation.
pub(crate) fn scan_backticks(
    rule: &dyn Rule,
    settings: &Settings,
    file: &File<'_>,
    taint: &FileTaint,
    message: impl Fn() -> Message,
) -> Option<Vec<Violation>> {
    let program = file.ast?;
    let mut violations = Vec::new();

    for statement in program.statements.iter() {
        walk_statement_expressions(statement, &mut |expression| {
            let Expression::CompositeString(string) = expression else {
                return;
            };
            if !matches!(string, CompositeString::ShellExecute(_)) {
                return;
            }
            let span = expression.span();
            let legacy = string
                .parts()
                .iter()
                .any(|part| matches!(part, StringPart::Expression(_) | StringPart::BracedExpression(_)));
            if !should_report(settings, taint, SinkKind::Shell, span, legacy) {
                return;
            }

            let mut violation = rule.new_violation(file, message(), span);
            annotate(settings, &mut violation, taint, SinkKind::Shell, span, file);
            violations.push(violation);
        });
    }

    Some(violations)
}

/// Walk every `new` expression and report tainted class names.
pub(crate) fn scan_instantiations(
    rule: &dyn Rule,
    settings: &Settings,
    file: &File<'_>,
    taint: &FileTaint,
    message: impl Fn() -> Message,
) -> Option<Vec<Violation>> {
    let program = file.ast?;
    let mut violations = Vec::new();

    for statement in program.statements.iter() {
        walk_statement_expressions(statement, &mut |expression| {
            let Expression::Instantiation(instantiation) = expression else {
                return;
            };
            let span = instantiation.span();
            let legacy = looks_dynamic(instantiation.class);
            if !should_report(settings, taint, SinkKind::Callable, span, legacy) {
                return;
            }

            let mut violation = rule.new_violation(file, message(), span);
            annotate(settings, &mut violation, taint, SinkKind::Callable, span, file);
            violations.push(violation);
        });
    }

    Some(violations)
}

/// Walk every `echo`, short echo tag and `print` and report unescaped output.
pub(crate) fn scan_output(
    rule: &dyn Rule,
    settings: &Settings,
    file: &File<'_>,
    taint: &FileTaint,
    message: impl Fn(&str) -> Message,
) -> Option<Vec<Violation>> {
    let program = file.ast?;
    let mut violations = Vec::new();
    let mut report = |value: &Expression<'_>, sink: &str, violations: &mut Vec<Violation>| {
        let span = value.span();
        let legacy = looks_dynamic(value);
        if !should_report(settings, taint, SinkKind::Html, span, legacy) {
            return;
        }
        let mut violation = rule.new_violation(file, message(sink), span);
        annotate(settings, &mut violation, taint, SinkKind::Html, span, file);
        violations.push(violation);
    };

    for statement in program.statements.iter() {
        for nested in rule.flatten_statements_to_validate(statement) {
            match nested {
                Statement::Echo(echo) => {
                    for value in echo.values.iter() {
                        report(value, "echo", &mut violations);
                    }
                }
                Statement::EchoTag(echo) => {
                    for value in echo.values.iter() {
                        report(value, "short-echo-tag", &mut violations);
                    }
                }
                _ => {}
            }
        }

        walk_statement_expressions(statement, &mut |expression| {
            if let Expression::Construct(Construct::Print(print_)) = expression {
                report(&print_.value, "print", &mut violations);
            }
        });
    }

    Some(violations)
}
