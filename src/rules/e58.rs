use std::collections::HashMap;

use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    function_name, literal_i64, literal_string, walk_statement_expressions,
};

pub(crate) static CODE: &str = "E0058";
static DESCRIPTION: &str = "Cookie missing Secure or HttpOnly";

#[derive(Default)]
pub struct Rule {}

impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        false
    }

    fn validate(&self, _file: &File<'_>, _statement: &Statement<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn validate_file(&self, file: &File<'_>) -> Option<Vec<Violation>> {
        let program = file.ast?;
        let mut violations = Vec::new();

        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| {
                let Expression::Call(Call::Function(call)) = expression else {
                    return;
                };
                let Some(name) = function_name(call.function) else {
                    return;
                };
                if !matches!(
                    name.as_str(),
                    "setcookie" | "setrawcookie" | "session_set_cookie_params"
                ) {
                    return;
                }

                let (secure, httponly) = if name == "session_set_cookie_params" {
                    session_cookie_flags(call)
                } else {
                    cookie_flags(call)
                };

                let mut missing = Vec::new();
                if secure == Some(false) {
                    missing.push("Secure");
                }
                if httponly == Some(false) {
                    missing.push("HttpOnly");
                }
                if missing.is_empty() {
                    return;
                }

                let message = Message::new(
                    format!("E0058:insecure-cookie:{name}"),
                    "{name}() is missing the {flags} flag. Set Secure and HttpOnly for session or authentication cookies.",
                )
                .arg("name", name)
                .arg("flags", missing.join(" and "));
                violations.push(self.new_violation(file, message, call.span()));
            });
        }

        Some(violations)
    }
}

fn cookie_flags(call: &FunctionCall<'_>) -> (Option<bool>, Option<bool>) {
    let (positional, named) = split_arguments(call);
    let options = named
        .get("expires_or_options")
        .copied()
        .or_else(|| positional.get(2).copied());

    (
        resolve_flag(&named, "secure", options, "secure", &positional, 5),
        resolve_flag(&named, "httponly", options, "httponly", &positional, 6),
    )
}

fn session_cookie_flags(call: &FunctionCall<'_>) -> (Option<bool>, Option<bool>) {
    let (positional, named) = split_arguments(call);
    let options = named
        .get("lifetime_or_options")
        .copied()
        .or_else(|| positional.first().copied());

    (
        resolve_flag(&named, "secure", options, "secure", &positional, 3),
        resolve_flag(&named, "httponly", options, "httponly", &positional, 4),
    )
}

fn split_arguments<'a>(
    call: &'a FunctionCall<'a>,
) -> (Vec<&'a Expression<'a>>, HashMap<String, &'a Expression<'a>>) {
    let mut positional = Vec::new();
    let mut named = HashMap::new();

    for argument in call.argument_list.arguments.iter() {
        match argument {
            Argument::Positional(argument) => positional.push(argument.value),
            Argument::Named(argument) => {
                let name = String::from_utf8_lossy(argument.name.value).to_ascii_lowercase();
                named.insert(name, argument.value);
            }
        }
    }

    (positional, named)
}

fn resolve_flag(
    named: &HashMap<String, &Expression<'_>>,
    name: &str,
    options: Option<&Expression<'_>>,
    array_key: &str,
    positional: &[&Expression<'_>],
    positional_index: usize,
) -> Option<bool> {
    if let Some(value) = named.get(name) {
        return literal_flag(value);
    }

    if let Some(options) = options {
        if let Some(value) = array_option(options, array_key) {
            return option_flag(value);
        }
        if !is_scalar_literal(options) {
            return None;
        }
    }

    positional_flag(positional.get(positional_index).copied())
}

fn array_option<'a>(
    expression: &'a Expression<'a>,
    key: &str,
) -> Option<Option<&'a Expression<'a>>> {
    let elements = match expression.unparenthesized() {
        Expression::Array(array) => &array.elements,
        Expression::LegacyArray(array) => &array.elements,
        _ => return None,
    };

    for element in elements.iter() {
        let ArrayElement::KeyValue(element) = element else {
            continue;
        };
        if literal_string(element.key)
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(key))
        {
            return Some(Some(element.value));
        }
    }

    Some(None)
}

fn option_flag(option: Option<&Expression<'_>>) -> Option<bool> {
    match option {
        None => Some(false),
        Some(expression) => literal_flag(expression),
    }
}

fn positional_flag(option: Option<&Expression<'_>>) -> Option<bool> {
    match option {
        None => Some(false),
        Some(expression) => literal_flag(expression),
    }
}

fn literal_flag(expression: &Expression<'_>) -> Option<bool> {
    match expression.unparenthesized() {
        Expression::Literal(Literal::True(_)) => Some(true),
        Expression::Literal(Literal::False(_)) => Some(false),
        _ => None,
    }
}

fn is_scalar_literal(expression: &Expression<'_>) -> bool {
    literal_i64(expression).is_some()
        || matches!(
            expression.unparenthesized(),
            Expression::Literal(
                Literal::String(_)
                    | Literal::Float(_)
                    | Literal::True(_)
                    | Literal::False(_)
                    | Literal::Null(_)
            )
        )
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_missing_secure_or_httponly_flags() {
        let violations = analyze_file_for_rule("e58/insecure_cookie_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_secure_httponly_cookies_and_dynamic_options() {
        assert!(analyze_file_for_rule("e58/insecure_cookie_safe.php", CODE).is_empty());
    }
}
