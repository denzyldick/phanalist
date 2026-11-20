use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use phanalist::config::Config;

fn temp_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("phanalist-init-{label}-{unique}"));
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("src/example.php"),
        "<?php\n\nfunction safe(int $value): int\n{\n    return $value;\n}\n",
    )
    .unwrap();
    root
}

fn phanalist() -> Command {
    Command::new(env!("CARGO_BIN_EXE_phanalist"))
}

#[test]
fn init_writes_a_usable_config_with_yes() {
    let root = temp_root("yes");
    let config = root.join("phanalist.yaml");
    let src = root.join("src");

    let result = phanalist()
        .args([
            "init",
            "--yes",
            "--config",
            config.to_str().unwrap(),
            "--src",
            src.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let loaded = Config::from_yaml_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(loaded.paths, vec![src.to_string_lossy().to_string()]);
    assert!(!fs::exists(root.join(".github/workflows/phanalist.yml")).unwrap());
    assert!(!fs::exists(root.join(".phanalist-baseline.json")).unwrap());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn init_refuses_to_overwrite_config_without_force() {
    let root = temp_root("force");
    let config = root.join("phanalist.yaml");
    let existing = "enabled_rules: []\ndisable_rules: []\nrules: {}\n";
    fs::write(&config, existing).unwrap();

    let result = phanalist()
        .args([
            "init",
            "--yes",
            "--config",
            config.to_str().unwrap(),
            "--src",
            root.join("src").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("already exists"));
    assert_eq!(fs::read_to_string(&config).unwrap(), existing);

    let result = phanalist()
        .args([
            "init",
            "--yes",
            "--force",
            "--config",
            config.to_str().unwrap(),
            "--src",
            root.join("src").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(config.with_extension("yaml.bak")).unwrap(),
        existing
    );

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn init_wires_generated_tests_baseline_and_ci() {
    let root = temp_root("wiring");
    let config = root.join("phanalist.yaml");
    let baseline = root.join("baselines/current.json");
    fs::write(
        root.join("phpunit.xml"),
        "<?xml version=\"1.0\"?>\n<phpunit>\n    <testsuites>\n    </testsuites>\n</phpunit>\n",
    )
    .unwrap();

    let result = phanalist()
        .args([
            "init",
            "--yes",
            "--config",
            config.to_str().unwrap(),
            "--src",
            root.join("src").to_str().unwrap(),
            "--generate-tests",
            "tests/generated",
            "--phpunit-config",
            "--create-baseline",
            baseline.to_str().unwrap(),
            "--ci",
            "--ci-events",
            "pull_request,push",
            "--ci-version",
            "v1.2.3",
            "--no-rule-descriptions",
            "--post-pr-comment",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );

    let loaded = Config::from_yaml_str(&fs::read_to_string(&config).unwrap()).unwrap();
    assert_eq!(
        loaded.test_generation.unwrap().output_dir,
        "tests/generated"
    );
    assert_eq!(
        loaded.baseline_path,
        Some(baseline.to_string_lossy().to_string())
    );

    let baseline_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&baseline).unwrap()).unwrap();
    assert_eq!(baseline_json["version"], 1);

    let phpunit = fs::read_to_string(root.join("phpunit.xml")).unwrap();
    assert!(phpunit.contains("<testsuite name=\"Phanalist Generated\">"));
    assert!(phpunit.contains("<directory>tests/generated</directory>"));

    let workflow = fs::read_to_string(root.join(".github/workflows/phanalist.yml")).unwrap();
    assert!(workflow.contains("  pull_request:\n"));
    assert!(workflow.contains("  push:\n"));
    assert!(workflow.contains("uses: denzyldick/phanalist-action@v1"));
    assert!(workflow.contains("version: 'v1.2.3'"));
    assert!(workflow.contains("include_rule_descriptions: 'false'"));
    assert!(workflow.contains("post_pr_comment: 'true'"));
    assert!(!workflow.contains("upload-sarif"));
    assert!(!workflow.contains("curl"));

    fs::remove_dir_all(&root).unwrap();
}
