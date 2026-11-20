use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("phanalist-{label}-{unique}"));
    fs::create_dir_all(path.join("src")).unwrap();
    fs::create_dir_all(path.join("tests")).unwrap();
    path
}

#[test]
fn generates_a_red_phpunit_test_from_a_solver_counterexample() {
    let root = temp_root("generated-tests");
    let source = root.join("src/divide.php");
    fs::write(
        &source,
        "<?php\n\nfunction divide(int $divisor): float\n{\n    return 1 / $divisor;\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("composer.json"),
        "{\n  \"name\": \"demo/app\"\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("phpunit.xml"),
        "<?xml version=\"1.0\"?>\n<phpunit>\n    <testsuites>\n    </testsuites>\n</phpunit>\n",
    )
    .unwrap();
    let output_dir = root.join("tests");

    // A violation makes the CLI exit with SOFTWARE; the files are what matter.
    let result = Command::new(env!("CARGO_BIN_EXE_phanalist"))
        .args([
            "-s",
            source.to_str().unwrap(),
            "--generate-tests",
            output_dir.to_str().unwrap(),
            "--rules",
            "E0031",
            "--output-format",
            "json",
        ])
        .output()
        .unwrap();

    let files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let file_stem = files[0].file_stem().unwrap().to_string_lossy();
    assert!(file_stem.ends_with("Test"), "{file_stem}");
    let generated = fs::read_to_string(&files[0]).unwrap();
    assert!(
        generated.contains(&format!("final class {file_stem} extends TestCase")),
        "{generated}"
    );
    assert!(generated.contains("\\divide(0)"), "{generated}");
    assert!(
        generated.contains("Solver inputs: $divisor = 0"),
        "{generated}"
    );
    assert!(
        generated.contains("use PHPUnit\\Framework\\TestCase;"),
        "{generated}"
    );
    assert!(generated.contains("require_once __DIR__"), "{generated}");
    assert!(generated.contains("set_error_handler"), "{generated}");
    assert!(generated.contains("assertTrue(true"), "{generated}");
    assert!(generated.contains("This test fails while"), "{generated}");
    assert!(!generated.contains("expectException"), "{generated}");

    let composer: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("composer.json")).unwrap()).unwrap();
    assert_eq!(
        composer["scripts"]["phanalist-tests"],
        serde_json::json!("phpunit tests")
    );
    let phpunit = fs::read_to_string(root.join("phpunit.xml")).unwrap();
    assert!(phpunit.contains("Phanalist Generated"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn generates_a_namespaced_static_method_test() {
    let root = temp_root("generated-static");
    let source = root.join("src/Calc.php");
    fs::write(
        &source,
        "<?php\n\nnamespace Demo;\n\nfinal class Calc\n{\n    public static function divide(int $d): float\n    {\n        return 1 / $d;\n    }\n}\n",
    )
    .unwrap();
    let output_dir = root.join("tests");

    let result = Command::new(env!("CARGO_BIN_EXE_phanalist"))
        .args([
            "-s",
            source.to_str().unwrap(),
            "--generate-tests",
            output_dir.to_str().unwrap(),
            "--rules",
            "E0031",
            "--output-format",
            "json",
        ])
        .output()
        .unwrap();

    let files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let generated = fs::read_to_string(&files[0]).unwrap();
    assert!(generated.contains("\\Demo\\Calc::divide(0)"), "{generated}");
    assert!(generated.contains("require_once __DIR__"), "{generated}");

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn witness_uses_the_call_entry_value_when_a_parameter_is_reassigned() {
    let root = temp_root("witness-entry-value");
    let source = root.join("src/reassign.php");
    fs::write(
        &source,
        "<?php\n\nfunction reassign(int $divisor): float\n{\n    $divisor = $divisor + 1;\n    return 1 / $divisor;\n}\n",
    )
    .unwrap();
    let output_dir = root.join("tests");

    let result = Command::new(env!("CARGO_BIN_EXE_phanalist"))
        .args([
            "-s",
            source.to_str().unwrap(),
            "--generate-tests",
            output_dir.to_str().unwrap(),
            "--rules",
            "E0031",
            "--output-format",
            "json",
        ])
        .output()
        .unwrap();

    let files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let generated = fs::read_to_string(&files[0]).unwrap();
    assert!(generated.contains("\\reassign(-1)"), "{generated}");
    assert!(
        generated.contains("Solver inputs: $divisor = -1"),
        "{generated}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn generates_tests_for_every_witness_capable_verification_rule() {
    let cases = [
        (
            "E0032",
            "ModuloByZero",
            "<?php\n\nfunction modulo(int $d): int\n{\n    return 10 % $d;\n}\n",
            "\\modulo(0)",
        ),
        (
            "E0033",
            "IntdivByZero",
            "<?php\n\nfunction intdivCase(int $d): int\n{\n    return intdiv(10, $d);\n}\n",
            "\\intdivCase(0)",
        ),
        (
            "E0034",
            "IntdivOverflow",
            "<?php\n\nfunction overflow(int $a, int $b): int\n{\n    return intdiv($a, $b);\n}\n",
            "\\overflow(-9223372036854775807 - 1, -1)",
        ),
        (
            "E0035",
            "MissingArrayKey",
            "<?php\n\nfunction keyCase(int $key): mixed\n{\n    $config = [\"a\" => 1];\n    return $config[$key];\n}\n",
            "\\keyCase(",
        ),
        (
            "E0036",
            "ArrayIndexOutOfBounds",
            "<?php\n\n/**\n * @param list<int> $items\n */\nfunction itemCase(array $items, int $index): int\n{\n    return $items[$index];\n}\n",
            "\\itemCase(array_fill(0, 0, null),",
        ),
        (
            "E0037",
            "StringOffsetOutOfBounds",
            "<?php\n\nfunction charCase(int $index): string\n{\n    $value = \"abc\";\n    return $value[$index];\n}\n",
            "\\charCase(",
        ),
        (
            "E0038",
            "NullMethodCall",
            "<?php\n\nclass Thing\n{\n    public function run(): void {}\n}\n\nfunction callOnNull(?Thing $thing): void\n{\n    $thing->run();\n}\n",
            "\\callOnNull(null)",
        ),
        (
            "E0039",
            "NullPropertyAccess",
            "<?php\n\nclass Box\n{\n    public int $value = 1;\n}\n\nfunction readNullProperty(?Box $box): int\n{\n    return $box->value;\n}\n",
            "\\readNullProperty(null)",
        ),
    ];

    for (index, (rule, label, source, invocation)) in cases.iter().enumerate() {
        let root = temp_root(&format!("rule-{index}"));
        let source_path = root.join("src/case.php");
        fs::write(&source_path, source).unwrap();
        let output_dir = root.join("tests");

        let result = Command::new(env!("CARGO_BIN_EXE_phanalist"))
            .args([
                "-s",
                source_path.to_str().unwrap(),
                "--generate-tests",
                output_dir.to_str().unwrap(),
                "--rules",
                rule,
                "--output-format",
                "json",
            ])
            .output()
            .unwrap();

        let files: Vec<_> = fs::read_dir(&output_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
            .collect();
        assert_eq!(
            files.len(),
            1,
            "{rule} stderr: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let generated = fs::read_to_string(&files[0]).unwrap();
        assert!(
            generated.contains(&format!("Rule: {rule} — {label}")),
            "{generated}"
        );
        assert!(generated.contains("Solver inputs:"), "{generated}");
        assert!(generated.contains(invocation), "{generated}");

        fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn skips_instance_methods_with_unconstructible_classes() {
    let root = temp_root("instance-method-skipped");
    let source = root.join("src/case.php");
    fs::write(
        &source,
        "<?php\n\nfinal class Calc\n{\n    private function __construct()\n    {\n    }\n\n    public function divide(int $d): float\n    {\n        return 1 / $d;\n    }\n}\n",
    )
    .unwrap();
    let output_dir = root.join("tests");

    let result = Command::new(env!("CARGO_BIN_EXE_phanalist"))
        .args([
            "-s",
            source.to_str().unwrap(),
            "--generate-tests",
            output_dir.to_str().unwrap(),
            "--rules",
            "E0031",
            "--output-format",
            "json",
        ])
        .output()
        .unwrap();
    let files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
        .collect();
    assert!(
        files.is_empty(),
        "unexpected files: {files:?}; stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn generates_an_instance_method_test_with_constructor_arguments() {
    let root = temp_root("instance-method");
    let source = root.join("src/case.php");
    fs::write(
        &source,
        "<?php\n\nnamespace Shop;\n\nfinal class Calc\n{\n    public function __construct(private string $label)\n    {\n    }\n\n    public function divide(int $divisor): float\n    {\n        return 1 / $divisor;\n    }\n}\n",
    )
    .unwrap();
    let output_dir = root.join("tests");

    let result = Command::new(env!("CARGO_BIN_EXE_phanalist"))
        .args([
            "-s",
            source.to_str().unwrap(),
            "--generate-tests",
            output_dir.to_str().unwrap(),
            "--rules",
            "E0031",
            "--output-format",
            "json",
        ])
        .output()
        .unwrap();

    let files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let generated = fs::read_to_string(&files[0]).unwrap();
    assert!(
        generated.contains("(new \\Shop\\Calc(str_repeat('a', 0)))->divide(0)"),
        "{generated}"
    );
    assert!(
        generated.contains("Receiver: new \\Shop\\Calc(str_repeat('a', 0))"),
        "{generated}"
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn generates_a_multi_rule_psr4_project() {
    let root = temp_root("psr4-project");
    let files = [
        (
            "src/Domain/PriceCalculator.php",
            "<?php\n\nnamespace Demo\\Shop\\Domain;\n\nfinal class PriceCalculator\n{\n    public static function discountedTotal(int $total, int $divisor): float\n    {\n        return $total / $divisor;\n    }\n}\n",
        ),
        (
            "src/Domain/DiscountRules.php",
            "<?php\n\nnamespace Demo\\Shop\\Domain;\n\nfunction applyModulo(int $divisor): int\n{\n    return 10 % $divisor;\n}\n\nfunction applyIntdiv(int $divisor): int\n{\n    return intdiv(10, $divisor);\n}\n\nfunction splitEvenly(int $dividend, int $divisor): int\n{\n    return intdiv($dividend, $divisor);\n}\n",
        ),
        (
            "src/Collections/Catalog.php",
            "<?php\n\nnamespace Demo\\Shop\\Collections;\n\nfunction readConfig(int $key): mixed\n{\n    $config = ['currency' => 'EUR'];\n\n    return $config[$key];\n}\n\n/**\n * @param list<int> $items\n */\nfunction readItem(array $items, int $index): int\n{\n    return $items[$index];\n}\n",
        ),
        (
            "src/Text/Slug.php",
            "<?php\n\nnamespace Demo\\Shop\\Text;\n\nfunction charAt(int $index): string\n{\n    $value = 'abc';\n\n    return $value[$index];\n}\n",
        ),
        (
            "src/Customer/Contact.php",
            "<?php\n\nnamespace Demo\\Shop\\Customer;\n\nclass Messenger\n{\n    public function send(): void\n    {\n    }\n}\n\nfunction notify(?Messenger $messenger): void\n{\n    $messenger->send();\n}\n",
        ),
        (
            "src/Customer/Profile.php",
            "<?php\n\nnamespace Demo\\Shop\\Customer;\n\nclass Settings\n{\n    public int $limit = 1;\n}\n\nfunction readLimit(?Settings $settings): int\n{\n    return $settings->limit;\n}\n",
        ),
        (
            "src/Billing/InvoiceCalculator.php",
            "<?php\n\nnamespace Demo\\Shop\\Billing;\n\nfinal class InvoiceCalculator\n{\n    public function __construct(private string $currency)\n    {\n    }\n\n    public function split(int $divisor): float\n    {\n        return 100 / $divisor;\n    }\n}\n",
        ),
    ];
    for (relative, source) in files {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }

    let output_dir = root.join("tests/generated");
    let mut command = Command::new(env!("CARGO_BIN_EXE_phanalist"));
    command.args(["-s", root.join("src").to_str().unwrap()]);
    for rule in [
        "E0031", "E0032", "E0033", "E0034", "E0035", "E0036", "E0037", "E0038", "E0039",
    ] {
        command.args(["--rules", rule]);
    }
    command.args([
        "--generate-tests",
        output_dir.to_str().unwrap(),
        "--output-format",
        "json",
    ]);
    let result = command.output().unwrap();

    let generated: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
        .collect();
    assert_eq!(
        generated.len(),
        11,
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let all_sources = generated
        .iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    for rule in [
        "E0031", "E0032", "E0033", "E0034", "E0035", "E0036", "E0037", "E0038", "E0039",
    ] {
        assert!(all_sources.contains(&format!("Rule: {rule} —")), "{rule}");
    }
    assert!(all_sources.contains("\\Demo\\Shop\\Domain\\PriceCalculator::discountedTotal"));
    assert!(all_sources
        .contains("(new \\Demo\\Shop\\Billing\\InvoiceCalculator(str_repeat('a', 0)))->split(0)"));

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn documents_inferred_route_to_service_call_chain() {
    let root = temp_root("call-chain");
    for dir in ["src/Controller", "src/Service"] {
        fs::create_dir_all(root.join(dir)).unwrap();
    }
    fs::write(
        root.join("src/Service/DiscountCalculator.php"),
        "<?php\n\nnamespace App\\Service;\n\nfinal class DiscountCalculator\n{\n    public function discountedTotal(int $total, int $divisor): float\n    {\n        return $total / $divisor;\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        root.join("src/Controller/CheckoutController.php"),
        "<?php\n\nnamespace App\\Controller;\n\nuse App\\Service\\DiscountCalculator;\nuse Symfony\\Component\\Routing\\Attribute\\Route;\n\nfinal class CheckoutController\n{\n    public function __construct(private DiscountCalculator $calculator)\n    {\n    }\n\n    #[Route('/checkout', methods: ['POST'])]\n    public function submit(): void\n    {\n        $this->calculator->discountedTotal(100, 0);\n    }\n}\n",
    )
    .unwrap();
    let output_dir = root.join("tests/generated");

    let result = Command::new(env!("CARGO_BIN_EXE_phanalist"))
        .current_dir(&root)
        .args([
            "-s",
            "src",
            "--rules",
            "E0031",
            "--generate-tests",
            "tests/generated",
            "--output-format",
            "json",
        ])
        .output()
        .unwrap();

    let files: Vec<_> = fs::read_dir(&output_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "php"))
        .collect();
    assert_eq!(
        files.len(),
        1,
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let generated = fs::read_to_string(&files[0]).unwrap();
    assert!(
        generated.contains("Call chain (inferred): POST /checkout"),
        "{generated}"
    );
    assert!(
        generated
            .contains("CheckoutController::submit() at src/Controller/CheckoutController.php:"),
        "{generated}"
    );
    assert!(
        generated.contains(
            "DiscountCalculator::discountedTotal() at src/Service/DiscountCalculator.php:"
        ),
        "{generated}"
    );

    let report = |format: &str| {
        Command::new(env!("CARGO_BIN_EXE_phanalist"))
            .current_dir(&root)
            .args(["-s", "src", "--rules", "E0031", "--output-format", format])
            .output()
            .unwrap()
    };

    let json: serde_json::Value = serde_json::from_slice(&report("json").stdout).unwrap();
    let json_violations: Vec<_> = json["files"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|violations| violations.as_array().unwrap())
        .collect();
    assert_eq!(json_violations.len(), 1, "{json}");
    assert_eq!(
        json_violations[0]["call_chain"],
        serde_json::json!([
            "POST /checkout → App\\Controller\\CheckoutController::submit() at src/Controller/CheckoutController.php:17 → App\\Service\\DiscountCalculator::discountedTotal() at src/Service/DiscountCalculator.php:9",
        ])
    );

    let sarif: serde_json::Value = serde_json::from_slice(&report("sarif").stdout).unwrap();
    let sarif_result = &sarif["runs"][0]["results"][0];
    assert!(sarif_result["message"]["text"]
        .as_str()
        .unwrap()
        .contains("Call chain (inferred): POST /checkout"));
    assert_eq!(sarif_result["properties"]["phanalist/callChain"][0], "POST /checkout → App\\Controller\\CheckoutController::submit() at src/Controller/CheckoutController.php:17 → App\\Service\\DiscountCalculator::discountedTotal() at src/Service/DiscountCalculator.php:9");

    let code_climate: serde_json::Value =
        serde_json::from_slice(&report("codeclimate").stdout).unwrap();
    assert_eq!(code_climate[0]["call_chain"][0], "POST /checkout → App\\Controller\\CheckoutController::submit() at src/Controller/CheckoutController.php:17 → App\\Service\\DiscountCalculator::discountedTotal() at src/Service/DiscountCalculator.php:9");
    assert!(code_climate[0]["content"]["body"]
        .as_str()
        .unwrap()
        .contains("Call chain (inferred): POST /checkout"));

    fs::remove_dir_all(&root).unwrap();
}
