use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::ast_support::{
    call_arguments, function_name, literal_string, walk_expression, walk_statement_expressions,
};

pub(crate) static CODE: &str = "E0057";
static DESCRIPTION: &str = "MD5 or SHA1 used for a password";

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
        let signature_spans = signature_spans(&program.statements);

        for statement in program.statements.iter() {
            walk_statement_expressions(statement, &mut |expression| {
                let Expression::Call(Call::Function(call)) = expression else {
                    return;
                };
                let Some(name) = function_name(call.function) else {
                    return;
                };

                let call = Call::Function(call.clone());
                let arguments = call_arguments(&call);

                let algorithm = match name.as_str() {
                    "md5" | "sha1" => name.as_str(),
                    "hash" => {
                        let Some(algorithm) = arguments
                            .first()
                            .and_then(|argument| literal_string(argument))
                        else {
                            return;
                        };
                        match algorithm.to_ascii_lowercase().as_str() {
                            "md5" => "md5",
                            "sha1" => "sha1",
                            _ => return,
                        }
                    }
                    _ => return,
                };

                let password_argument = if name == "hash" {
                    arguments.get(1)
                } else {
                    arguments.first()
                };
                let Some(password_argument) = password_argument else {
                    return;
                };
                if !contains_password_variable(password_argument) {
                    return;
                }

                let span = call.span();
                if signature_spans
                    .iter()
                    .any(|(start, end)| span.start.offset >= *start && span.start.offset < *end)
                {
                    return;
                }

                let message = Message::new(
                    format!("E0057:weak-password-hash:{algorithm}"),
                    "{algorithm} is not suitable for password hashing. Use password_hash() with PASSWORD_DEFAULT or PASSWORD_ARGON2ID.",
                )
                .arg("algorithm", algorithm);
                violations.push(self.new_violation(file, message, call.span()));
            });
        }

        Some(violations)
    }
}

fn signature_spans(statements: &Sequence<'_, Statement<'_>>) -> Vec<(u32, u32)> {
    let mut spans = Vec::new();

    for statement in statements.iter() {
        match statement {
            Statement::Namespace(namespace) => {
                spans.extend(signature_spans(namespace.statements()));
            }
            Statement::Function(function) => {
                if is_signature_name(&String::from_utf8_lossy(function.name.value)) {
                    spans.push(span_range(function.body.span()));
                }
            }
            Statement::Class(class) => collect_member_signature_spans(&class.members, &mut spans),
            Statement::Interface(interface) => {
                collect_member_signature_spans(&interface.members, &mut spans)
            }
            Statement::Trait(r#trait) => {
                collect_member_signature_spans(&r#trait.members, &mut spans)
            }
            Statement::Enum(r#enum) => collect_member_signature_spans(&r#enum.members, &mut spans),
            _ => {}
        }
    }

    spans
}

fn collect_member_signature_spans(
    members: &Sequence<'_, ClassLikeMember<'_>>,
    spans: &mut Vec<(u32, u32)>,
) {
    for member in members.iter() {
        let ClassLikeMember::Method(method) = member else {
            continue;
        };
        if !is_signature_name(&String::from_utf8_lossy(method.name.value)) {
            continue;
        }
        if let MethodBody::Concrete(body) = &method.body {
            spans.push(span_range(body.span()));
        }
    }
}

fn is_signature_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("sign") || name.contains("hmac")
}

fn span_range(span: Span) -> (u32, u32) {
    (span.start.offset, span.end.offset)
}

fn contains_password_variable(expression: &Expression<'_>) -> bool {
    let mut found = false;
    walk_expression(expression, &mut |expression| {
        if found {
            return;
        }

        match expression {
            Expression::Variable(Variable::Direct(variable)) => {
                if let Ok(name) = std::str::from_utf8(variable.name) {
                    found = is_password_name(name);
                }
            }
            Expression::Access(Access::Property(access)) => {
                if let Some(name) = crate::rules::ast_support::method_name(&access.property) {
                    found = is_password_name(&name);
                }
            }
            Expression::Access(Access::NullSafeProperty(access)) => {
                if let Some(name) = crate::rules::ast_support::method_name(&access.property) {
                    found = is_password_name(&name);
                }
            }
            _ => {}
        }
    });
    found
}

fn is_password_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["password", "passwd", "passphrase", "pwd"]
        .iter()
        .any(|part| name.contains(part))
        || name == "pass"
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_md5_and_sha1_password_hashes() {
        let violations = analyze_file_for_rule("e57/weak_password_hash_bad.php", CODE);
        assert!(!violations.is_empty());
        assert!(violations.iter().all(|violation| violation.rule == CODE));
    }

    #[test]
    fn accepts_password_hash_and_non_password_digests() {
        assert!(analyze_file_for_rule("e57/weak_password_hash_safe.php", CODE).is_empty());
    }

    #[test]
    fn accepts_password_in_signature_helpers() {
        assert!(analyze_file_for_rule("e57/signature_context_safe.php", CODE).is_empty());
    }
}
