use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_arguments, call_span, function_name, literal_array_flag, literal_bool,
    walk_statement_expressions,
};

pub(crate) static CODE: &str = "E0097";
static DESCRIPTION: &str = "Cookie without HttpOnly flag";

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
                let Expression::Call(call) = expression else {
                    return;
                };
                let Some(name) = function_name(match call {
                    Call::Function(call) => call.function,
                    _ => return,
                }) else {
                    return;
                };

                if !is_cookie_function(&name) {
                    return;
                }

                let arguments = call_arguments(call);
                if has_flag(&name, &arguments, "httponly") {
                    return;
                }

                let message = Message::new(
                    format!("E0097:cookie-no-httponly:{name}"),
                    "{}() sets a cookie without the HttpOnly flag. Add HttpOnly so client-side scripts cannot read the cookie.",
                )
                .arg("name", name);

                violations.push(self.new_violation(file, message, call_span(call)));
            });
        }

        Some(violations)
    }
}

fn is_cookie_function(name: &str) -> bool {
    matches!(
        name,
        "setcookie" | "setrawcookie" | "session_set_cookie_params"
    )
}

/// Whether the call explicitly sets `flag` to `true`.
fn has_flag(name: &str, arguments: &[&Expression<'_>], flag: &str) -> bool {
    let (options_index, positional_index) = if name == "session_set_cookie_params" {
        (0, 4)
    } else {
        (2, 6)
    };

    if let Some(options) = arguments.get(options_index) {
        if let Some(flag_is_true) = literal_array_flag(options, flag) {
            return flag_is_true;
        }
    }

    arguments
        .get(positional_index)
        .is_some_and(|argument| literal_bool(argument) == Some(true))
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_cookie_without_httponly_flag() {
        let violations = analyze_file_for_rule("e97/bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|v| v.rule == CODE));
    }

    #[test]
    fn accepts_safe() {
        assert!(analyze_file_for_rule("e97/safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_an_options_array_with_httponly_true() {
        assert!(analyze_file(
            "<?php\nsetcookie('session', 'v', ['secure' => true, 'httponly' => true]);\n"
        )
        .is_empty());
    }

    #[test]
    fn reports_an_options_array_with_httponly_false() {
        let violations = analyze_file(
            "<?php\nsetcookie('session', 'v', ['secure' => true, 'httponly' => false]);\n",
        );
        assert_eq!(violations.len(), 1);
    }

    fn analyze_file(source: &str) -> Vec<Violation> {
        crate::rules::tests::analyze_source_for_rule(source, "inline.php", CODE)
    }
}
