//! Render solver counterexamples as PHPUnit regression tests.
//!
//! The generated test is intentionally red while the reported bug exists: it
//! calls the real function with the solver's inputs and asserts that the call
//! completes without a PHP error or warning. PHPUnit 10+ does not fail tests on
//! warnings by default, so a short local error handler converts them to
//! exceptions. A successful run reaches the final `assertTrue(true, ...)`.

use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

use crate::results::GeneratedTest;

use super::value::ArrayKey;
use super::witness::{CallableKind, ConcreteValue, Witness};

/// Render one finding's witness as a test file. Returns `None` when the model
/// did not pin down an input well enough to write a faithful PHP literal.
pub fn generate_test(witness: &Witness, output_dir: Option<&Path>) -> Option<GeneratedTest> {
    let callable = witness.callable.as_ref()?;
    if witness.has_unsupported_input() {
        return None;
    }
    let arguments = witness
        .inputs
        .iter()
        .map(|input| php_literal(&input.value))
        .collect::<Option<Vec<_>>>()?;

    let invocation = match callable.kind {
        CallableKind::Function | CallableKind::StaticMethod => format!(
            "\\{}({})",
            callable.name.trim_start_matches('\\'),
            arguments.join(", ")
        ),
        CallableKind::InstanceMethod => {
            let receiver = witness.receiver.as_ref()?;
            let receiver_arguments = receiver
                .arguments
                .iter()
                .map(|input| php_literal(&input.value))
                .collect::<Option<Vec<_>>>()?
                .join(", ");
            let method = callable.name.rsplit("::").next().unwrap_or(&callable.name);
            format!(
                "(new \\{}({}))->{}({})",
                receiver.class.trim_start_matches('\\'),
                receiver_arguments,
                method,
                arguments.join(", ")
            )
        }
    };

    let source_path = witness.source.to_string_lossy();
    let receiver_hash = witness
        .receiver
        .as_ref()
        .map(|receiver| {
            format!(
                "{}:{}",
                receiver.class,
                receiver
                    .arguments
                    .iter()
                    .map(|input| format!("{}={:?}", input.name, input.value))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .unwrap_or_default();
    let hash = short_hash(&format!(
        "{}:{}:{}:{}:{}",
        source_path,
        witness.line,
        witness.code,
        witness
            .inputs
            .iter()
            .map(|input| format!("{}={:?}", input.name, input.value))
            .collect::<Vec<_>>()
            .join(","),
        receiver_hash
    ));
    let callable_part = pascal_identifier(&short_callable_name(&callable.name));
    // PHPUnit discovers a file by expecting a class named after its basename,
    // and directory suites only pick up `*Test.php`. Keep the collision-safe
    // suffix but make the file name exactly match the class name.
    let class_name = format!(
        "{}{}Line{}{:08X}Test",
        callable_part,
        rule_label(&witness.code),
        witness.line,
        hash
    );
    let test_name = format!(
        "test{}{}Line{}",
        callable_part,
        rule_label(&witness.code),
        witness.line
    );
    let file_name = format!("{class_name}.php");

    let output_dir =
        output_dir.unwrap_or_else(|| witness.source.parent().unwrap_or(Path::new(".")));
    let path = output_dir.join(file_name);
    let source_require = require_expr(output_dir, &witness.source);
    let autoload_require = find_autoload(output_dir)
        .map(|autoload| format!("require_once {};\n", require_expr(output_dir, &autoload)))
        .unwrap_or_default();
    let inputs = witness
        .inputs
        .iter()
        .map(|input| {
            format!(
                "{} = {}",
                input.name,
                php_literal(&input.value).unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let receiver_doc = witness
        .receiver
        .as_ref()
        .map(|receiver| {
            let arguments = receiver
                .arguments
                .iter()
                .map(|input| php_literal(&input.value).unwrap_or_default())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                " * Receiver: new \\{}({arguments})\n",
                receiver.class.trim_start_matches('\\')
            )
        })
        .unwrap_or_default();
    let call_chain_doc = witness
        .call_chain
        .first()
        .map(|chain| format!(" * Call chain (inferred): {}\n", docblock_safe(chain)))
        .unwrap_or_default();
    let evidence_doc = format!("{receiver_doc}{call_chain_doc}");

    let source = format!(
        r#"<?php

declare(strict_types=1);

use PHPUnit\Framework\TestCase;

/**
 * Generated by phanalist. Review before committing.
 *
 * Rule: {code} — {label}
 * Source: {source}:{line}
 * Finding: {message}
 * Solver inputs: {inputs}
{evidence_doc} *
 * This test fails while the reported path is still reachable.
 */
{autoload_require}require_once {source_require};

final class {class_name} extends TestCase
{{
    public function {test_name}(): void
    {{
        set_error_handler(static function (int $severity, string $message): bool {{
            throw new \ErrorException($message, 0, $severity);
        }});

        try {{
            {invocation};
            $this->assertTrue(true, 'The solver inputs no longer trigger a PHP error.');
        }} finally {{
            restore_error_handler();
        }}
    }}
}}
"#,
        code = docblock_safe(&witness.code),
        label = docblock_safe(rule_label(&witness.code)),
        source = docblock_safe(&source_path),
        line = witness.line,
        message = docblock_safe(&witness.message),
        inputs = docblock_safe(&inputs),
        evidence_doc = evidence_doc,
        autoload_require = autoload_require,
        source_require = source_require,
        class_name = class_name,
        test_name = test_name,
        invocation = invocation,
    );

    Some(GeneratedTest {
        path,
        source,
        test_name,
        rule: witness.code.clone(),
        line: witness.line,
        witness: witness.clone(),
    })
}

fn php_literal(value: &ConcreteValue) -> Option<String> {
    match value {
        ConcreteValue::Int(value) if *value == i64::MIN => {
            Some("-9223372036854775807 - 1".to_string())
        }
        ConcreteValue::Int(value) => Some(value.to_string()),
        ConcreteValue::Float(value) => Some(format!("{value:?}")),
        ConcreteValue::Bool(value) => Some(value.to_string()),
        ConcreteValue::StringOfLength(length) => Some(format!("str_repeat('a', {length})")),
        ConcreteValue::StringLiteral(value) => Some(php_string(value)),
        ConcreteValue::ArrayOfLength(length) => Some(format!("array_fill(0, {length}, null)")),
        ConcreteValue::ArrayKeys(keys) => Some(format!(
            "[{}]",
            keys.iter()
                .map(|key| match key {
                    ArrayKey::Int(index) => format!("{index} => null"),
                    ArrayKey::String(key) => format!("{} => null", php_string(key)),
                })
                .collect::<Vec<_>>()
                .join(", ")
        )),
        ConcreteValue::Null => Some("null".to_string()),
        ConcreteValue::Unsupported(_) => None,
    }
}

fn rule_label(code: &str) -> &'static str {
    match code {
        "E0031" => "DivisionByZero",
        "E0032" => "ModuloByZero",
        "E0033" => "IntdivByZero",
        "E0034" => "IntdivOverflow",
        "E0035" => "MissingArrayKey",
        "E0036" => "ArrayIndexOutOfBounds",
        "E0037" => "StringOffsetOutOfBounds",
        "E0038" => "NullMethodCall",
        "E0039" => "NullPropertyAccess",
        "E0040" => "ReturnContract",
        "E0041" => "UnreachableCode",
        "E0042" => "ImpossibleAssertion",
        "E0043" => "StreamLifecycle",
        _ => "Verification",
    }
}

fn short_callable_name(name: &str) -> String {
    name.rsplit(['\\', ':'])
        .next()
        .unwrap_or(name)
        .trim_start_matches('$')
        .to_string()
}

fn php_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn docblock_safe(value: &str) -> String {
    value.replace("*/", "* /").replace('\n', " ")
}

fn sanitize_identifier(value: &str) -> String {
    let mut out: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.as_bytes()[0].is_ascii_digit() {
        out.insert(0, '_');
    }
    out
}

fn pascal_identifier(value: &str) -> String {
    let mut out = sanitize_identifier(value);
    if let Some(first) = out.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    out
}

fn short_hash(value: &str) -> u32 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish() as u32
}

fn absolutize(path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn require_expr(from_dir: &Path, to_file: &Path) -> String {
    let relative = relative_path(from_dir, to_file);
    if relative.is_absolute() {
        php_string(&relative.to_string_lossy())
    } else {
        format!(
            "__DIR__ . {}",
            php_string(&format!("/{}", relative.display()))
        )
    }
}

fn find_autoload(output_dir: &Path) -> Option<PathBuf> {
    let start = absolutize(output_dir);
    let mut current = Some(start.as_path());
    while let Some(dir) = current {
        let autoload = dir.join("vendor").join("autoload.php");
        if autoload.is_file() {
            return Some(autoload);
        }
        current = dir.parent();
    }
    None
}

fn relative_path(from_dir: &Path, to_file: &Path) -> PathBuf {
    let from = absolutize(from_dir);
    let to = absolutize(to_file);
    let common = from
        .components()
        .zip(to.components())
        .take_while(|(left, right)| left == right)
        .count();
    let mut out = PathBuf::new();
    for _ in common..from.components().count() {
        out.push("..");
    }
    for component in to.components().skip(common) {
        out.push(component.as_os_str());
    }
    if out.as_os_str().is_empty() {
        to
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use crate::verification::witness::{CallableContext, WitnessInput, WitnessReceiver};

    use super::*;

    fn witness(value: ConcreteValue) -> Witness {
        Witness {
            code: "E0031".to_string(),
            message: "Division by zero: the divisor can be 0.".to_string(),
            source: PathBuf::from("/project/src/Calc.php"),
            line: 12,
            callable: Some(CallableContext {
                name: "Example\\divide".to_string(),
                kind: CallableKind::Function,
                file: PathBuf::from("/project/src/Calc.php"),
                line: 4,
            }),
            inputs: vec![WitnessInput {
                name: "$divisor".to_string(),
                value,
            }],
            receiver: None,
            call_chain: Vec::new(),
        }
    }

    #[test]
    fn renders_a_red_phpunit_test_with_the_witness_inputs() {
        let generated = generate_test(&witness(ConcreteValue::Int(0)), None).unwrap();
        assert!(generated.source.contains("\\Example\\divide(0)"));
        assert!(generated.source.contains("set_error_handler"));
        assert!(generated.source.contains("assertTrue(true"));
        assert!(!generated.source.contains("expectException"));
        assert!(generated.source.contains("E0031 — DivisionByZero"));
        assert!(generated.test_name == "testDivideDivisionByZeroLine12");
        assert!(generated
            .source
            .contains("final class DivideDivisionByZeroLine12"));
    }

    #[test]
    fn file_name_matches_the_class_so_phpunit_can_discover_it() {
        let generated = generate_test(&witness(ConcreteValue::Int(0)), None).unwrap();
        let stem = generated.path.file_stem().unwrap().to_string_lossy();
        assert!(stem.ends_with("Test"));
        assert!(generated.path.to_string_lossy().ends_with("Test.php"));
        assert!(generated
            .source
            .contains(&format!("final class {stem} extends TestCase")));
    }

    #[test]
    fn renders_a_namespaced_static_method_call() {
        let mut found = witness(ConcreteValue::Int(0));
        found.callable = Some(CallableContext {
            name: "Demo\\Calc::divide".to_string(),
            kind: CallableKind::StaticMethod,
            file: PathBuf::from("/project/src/Calc.php"),
            line: 4,
        });
        let generated = generate_test(&found, None).unwrap();
        assert!(generated.source.contains("\\Demo\\Calc::divide(0)"));
        assert!(generated
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("Divide"));
    }

    #[test]
    fn renders_an_instance_method_with_its_constructor_arguments() {
        let mut found = witness(ConcreteValue::Int(0));
        found.callable = Some(CallableContext {
            name: "Demo\\Calc::divide".to_string(),
            kind: CallableKind::InstanceMethod,
            file: PathBuf::from("/project/src/Calc.php"),
            line: 4,
        });
        found.receiver = Some(WitnessReceiver {
            class: "Demo\\Calc".to_string(),
            arguments: vec![WitnessInput {
                name: "$label".to_string(),
                value: ConcreteValue::StringOfLength(0),
            }],
        });

        let generated = generate_test(&found, None).unwrap();
        assert!(generated
            .source
            .contains("(new \\Demo\\Calc(str_repeat('a', 0)))->divide(0)"));
        assert!(generated
            .source
            .contains("Receiver: new \\Demo\\Calc(str_repeat('a', 0))"));
    }

    #[test]
    fn renders_i64_min_as_a_valid_php_expression() {
        let generated = generate_test(&witness(ConcreteValue::Int(i64::MIN)), None).unwrap();
        assert!(generated
            .source
            .contains("\\Example\\divide(-9223372036854775807 - 1)"));
    }

    #[test]
    fn escapes_docblock_terminators_in_finding_text() {
        let mut found = witness(ConcreteValue::Int(0));
        found.message = "Unexpected */ tokens\nstay on one line".to_string();
        let generated = generate_test(&found, None).unwrap();
        assert!(generated
            .source
            .contains("Unexpected * / tokens stay on one line"));
        assert!(!generated.source.contains("Unexpected */"));
    }

    #[test]
    fn different_counterexamples_get_different_class_names() {
        let first = generate_test(&witness(ConcreteValue::Int(0)), None).unwrap();
        let second = generate_test(&witness(ConcreteValue::Int(1)), None).unwrap();
        assert_ne!(first.path.file_name(), second.path.file_name());
    }

    #[test]
    fn renders_lengths_and_booleans() {
        let generated = generate_test(&witness(ConcreteValue::StringOfLength(3)), None).unwrap();
        assert!(generated
            .source
            .contains("\\Example\\divide(str_repeat('a', 3))"));
        let generated = generate_test(&witness(ConcreteValue::Bool(true)), None).unwrap();
        assert!(generated.source.contains("\\Example\\divide(true)"));
    }

    #[test]
    fn renders_container_placeholders_and_null() {
        let generated = generate_test(&witness(ConcreteValue::ArrayOfLength(0)), None).unwrap();
        assert!(generated
            .source
            .contains("\\Example\\divide(array_fill(0, 0, null))"));
        let generated = generate_test(
            &witness(ConcreteValue::ArrayKeys(vec![ArrayKey::Int(1)])),
            None,
        )
        .unwrap();
        assert!(generated.source.contains("\\Example\\divide([1 => null])"));
        let generated =
            generate_test(&witness(ConcreteValue::StringLiteral("a'b".into())), None).unwrap();
        assert!(generated.source.contains("\\Example\\divide('a\\'b')"));
        let generated = generate_test(&witness(ConcreteValue::Null), None).unwrap();
        assert!(generated.source.contains("\\Example\\divide(null)"));
    }

    #[test]
    fn refuses_unrenderable_inputs() {
        assert!(generate_test(&witness(ConcreteValue::Unsupported("?".into())), None).is_none());
    }

    #[test]
    fn uses_a_relative_require_path() {
        let generated = generate_test(
            &witness(ConcreteValue::Int(0)),
            Some(Path::new("/project/tests/phanalist")),
        )
        .unwrap();
        assert!(generated
            .source
            .contains("require_once __DIR__ . '/../../src/Calc.php'"));
    }
}
