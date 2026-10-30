use std::collections::HashSet;

use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_arguments, direct_variable_name, function_name, literal_string, walk_expression,
    walk_statement_expressions,
};
use crate::rules::Rule as _;

pub(crate) static CODE: &str = "E0056";
static DESCRIPTION: &str = "Weak randomness for security-sensitive value";

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
        let mut seen = HashSet::new();

        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| match expression {
                Expression::Assignment(assignment) if assignment.operator.is_assign() => {
                    let Some(variable) = direct_variable_name(assignment.lhs) else {
                        return;
                    };
                    if !is_sensitive_name(variable) {
                        return;
                    }

                    if let Some(span) = find_weak_random(assignment.rhs) {
                        self.report(
                            file,
                            &mut violations,
                            &mut seen,
                            span,
                            "assigned to a security-sensitive variable",
                        );
                    }
                }
                Expression::Call(Call::Function(call)) => {
                    let Some(name) = function_name(call.function) else {
                        return;
                    };
                    if !is_sensitive_sink(&name) {
                        return;
                    }

                    let call = Call::Function(call.clone());
                    let arguments = call_arguments(&call);
                    if is_hash_function(&name)
                        && arguments
                            .first()
                            .and_then(|argument| literal_string(argument))
                            .is_some_and(|algorithm| is_non_cryptographic_hash(&algorithm))
                    {
                        return;
                    }
                    for argument in arguments {
                        if let Some(span) = find_weak_random(argument) {
                            self.report(
                                file,
                                &mut violations,
                                &mut seen,
                                span,
                                &format!("passed to {name}()"),
                            );
                        }
                    }
                }
                _ => {}
            });
        }

        Some(violations)
    }
}

impl Rule {
    fn report(
        &self,
        file: &File<'_>,
        violations: &mut Vec<Violation>,
        seen: &mut HashSet<u32>,
        span: Span,
        context: &str,
    ) {
        if !seen.insert(span.start.offset) {
            return;
        }

        let message = Message::new(
            "E0056:weak-randomness:security-sensitive",
            "Weak randomness is used for a security-sensitive value. Use random_bytes() or random_int() instead of rand(), mt_rand(), uniqid(), or similar functions.",
        )
        .arg("context", context);
        violations.push(self.new_violation(file, message, span));
    }
}

fn find_weak_random<'a>(expression: &'a Expression<'a>) -> Option<Span> {
    let mut found = None;
    walk_expression(expression, &mut |expression| {
        if found.is_some() {
            return;
        }
        let Expression::Call(Call::Function(call)) = expression else {
            return;
        };
        let Some(name) = function_name(call.function) else {
            return;
        };
        if is_weak_random(&name) {
            found = Some(call.span());
        }
    });
    found
}

fn is_weak_random(name: &str) -> bool {
    matches!(
        name,
        "rand" | "mt_rand" | "uniqid" | "lcg_value" | "str_shuffle" | "array_rand"
    )
}

fn is_hash_function(name: &str) -> bool {
    matches!(name, "hash" | "hash_hmac")
}

fn is_non_cryptographic_hash(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("xxh")
        || name.starts_with("murmur")
        || matches!(
            name.as_str(),
            "crc32"
                | "crc32b"
                | "crc32c"
                | "adler32"
                | "fnv132"
                | "fnv1a32"
                | "fnv164"
                | "fnv1a64"
                | "joaat"
        )
}

fn is_sensitive_sink(name: &str) -> bool {
    matches!(
        name,
        "password_hash"
            | "hash_hmac"
            | "hash"
            | "openssl_encrypt"
            | "openssl_sign"
            | "setcookie"
            | "setrawcookie"
            | "session_id"
            | "session_create_id"
    )
}

fn is_sensitive_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "passwd",
        "pwd",
        "nonce",
        "salt",
        "csrf",
        "api_key",
        "apikey",
        "private_key",
        "privatekey",
        "signing_key",
        "signingkey",
        "verification_code",
        "verificationcode",
        "otp",
    ]
    .iter()
    .any(|part| name.contains(part))
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_weak_randomness_in_security_contexts() {
        let violations = analyze_file_for_rule("e56/weak_randomness_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_secure_randomness_and_non_security_uses() {
        assert!(analyze_file_for_rule("e56/weak_randomness_safe.php", CODE).is_empty());
        assert!(analyze_file_for_rule("e56/non_crypto_hash_safe.php", CODE).is_empty());
    }
}
