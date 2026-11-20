//! Writing generated regression tests and wiring them into the project.
//!
//! Phanalist never executes generated tests (or any user PHP code). It only
//! writes files and, when present, patches `composer.json` and the PHPUnit
//! configuration so the tests are easy to run.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::results::Results;

const COMPOSER_SCRIPT: &str = "phanalist-tests";
const TESTSUITE_NAME: &str = "Phanalist Generated";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct WriteSummary {
    pub written: usize,
    pub composer_updated: bool,
    pub phpunit_updated: bool,
}

/// Write every generated test attached to `results` into the output directory.
pub fn write_generated_tests(results: &mut Results, output_dir: &Path) -> io::Result<WriteSummary> {
    fs::create_dir_all(output_dir)?;

    let mut summary = WriteSummary::default();
    for file in results.files.values_mut() {
        for violation in file.iter_mut() {
            let Some(test) = violation.generated_test.take() else {
                continue;
            };
            fs::write(&test.path, test.source.as_bytes())?;
            summary.written += 1;
            violation.generated_test = Some(test);
        }
    }

    if let Some(composer) = find_upwards(output_dir, "composer.json") {
        summary.composer_updated = add_composer_script(&composer, output_dir)?;
    }
    let phpunit_config = find_upwards(output_dir, "phpunit.xml")
        .or_else(|| find_upwards(output_dir, "phpunit.xml.dist"));
    if let Some(config) = phpunit_config {
        summary.phpunit_updated = add_phpunit_testsuite(&config, output_dir)?;
    }

    Ok(summary)
}

fn find_upwards(start: &Path, file_name: &str) -> Option<PathBuf> {
    let start = absolutize(start);
    let mut current = Some(start.as_path());
    while let Some(dir) = current {
        let candidate = dir.join(file_name);
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

fn add_composer_script(composer: &Path, output_dir: &Path) -> io::Result<bool> {
    let original = fs::read_to_string(composer)?;
    let value: serde_json::Value = serde_json::from_str(&original)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if value
        .get("scripts")
        .and_then(|scripts| scripts.get(COMPOSER_SCRIPT))
        .is_some()
    {
        return Ok(false);
    }

    let dir = composer.parent().unwrap_or_else(|| Path::new("."));
    let command = format!(
        "phpunit {}",
        relative_path(dir, &absolutize(output_dir)).display()
    );
    let entry = format!(
        "{}: {}",
        serde_json::to_string(COMPOSER_SCRIPT).unwrap(),
        serde_json::to_string(&command).unwrap()
    );
    let updated = if let Some((open, close)) = object_span(&original, "\"scripts\"") {
        let inner = &original[open + 1..close];
        let first = inner.trim_start();
        let indent = line_indent(&original, open)
            .map(|indent| format!("{indent}  "))
            .unwrap_or_else(|| "    ".to_string());
        if first.is_empty() {
            format!(
                "{}\n{}{}\n{}{}",
                &original[..open + 1],
                indent,
                entry,
                line_indent(&original, open).unwrap_or("  "),
                &original[close..]
            )
        } else {
            format!(
                "{},\n{}{}{}",
                &original[..close],
                indent,
                entry,
                &original[close..]
            )
        }
    } else {
        let scripts = serde_json::json!({ COMPOSER_SCRIPT: command });
        insert_top_level_json(&original, "scripts", &scripts)
    };

    serde_json::from_str::<serde_json::Value>(&updated)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(composer, updated)?;
    Ok(true)
}

fn insert_top_level_json(original: &str, key: &str, value: &serde_json::Value) -> String {
    let Some(last_brace) = original.rfind('}') else {
        return original.to_string();
    };
    let prefix = &original[..last_brace];
    let trimmed = prefix.trim_end();
    let comma = if trimmed.ends_with('{') || trimmed.ends_with(',') {
        ""
    } else {
        ","
    };
    let pretty = serde_json::to_string_pretty(value).unwrap();
    let indented = pretty
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if index == 0 {
                line.to_string()
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{}{}\n  {}: {}\n}}",
        trimmed,
        comma,
        serde_json::to_string(key).unwrap(),
        indented
    )
}

fn object_span(text: &str, key: &str) -> Option<(usize, usize)> {
    let key_pos = text.find(key)?;
    let colon = text[key_pos + key.len()..].find(':')? + key_pos + key.len();
    let after_colon = text[colon + 1..]
        .find('{')
        .map(|offset| colon + 1 + offset)?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, character) in text.char_indices().skip(after_colon) {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((after_colon, offset));
                }
            }
            _ => {}
        }
    }
    None
}

fn line_indent(text: &str, offset: usize) -> Option<&str> {
    let start = text[..offset].rfind('\n').map(|pos| pos + 1).unwrap_or(0);
    let line = &text[start..offset];
    let indent_len = line
        .chars()
        .take_while(|character| character.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    Some(&line[..indent_len])
}

/// Add the generated-test directory to an existing PHPUnit configuration.
/// Safe to call repeatedly; returns `true` only when the file changed.
pub fn add_phpunit_testsuite(config: &Path, output_dir: &Path) -> io::Result<bool> {
    let original = fs::read_to_string(config)?;
    if original.contains(TESTSUITE_NAME) {
        return Ok(false);
    }
    let dir = config.parent().unwrap_or_else(|| Path::new("."));
    let relative = relative_path(dir, &absolutize(output_dir));
    let entry = format!(
        "    <testsuite name=\"{}\">\n        <directory>{}</directory>\n    </testsuite>\n",
        TESTSUITE_NAME,
        xml_escape(&relative.to_string_lossy())
    );
    let updated = if let Some(pos) = original.rfind("</testsuites>") {
        format!("{}{}{}", &original[..pos], entry, &original[pos..])
    } else if let Some(pos) = original.rfind("</phpunit>") {
        format!(
            "{}    <testsuites>\n{}    </testsuites>\n{}",
            &original[..pos],
            entry,
            &original[pos..]
        )
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PHPUnit config has no </phpunit> element",
        ));
    };
    fs::write(config, updated)?;
    Ok(true)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::results::Violation;
    use crate::verification::witness::{
        CallableContext, CallableKind, ConcreteValue, Witness, WitnessInput,
    };

    use super::*;

    fn temp_root() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("phanalist-writer-{unique}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn results_with_test(path: &Path) -> Results {
        let witness = Witness {
            code: "E0031".to_string(),
            message: "Division by zero".to_string(),
            source: path.join("src/example.php"),
            line: 3,
            callable: Some(CallableContext {
                name: "divide".to_string(),
                kind: CallableKind::Function,
                file: path.join("src/example.php"),
                line: 2,
            }),
            inputs: vec![WitnessInput {
                name: "$d".to_string(),
                value: ConcreteValue::Int(0),
            }],
            receiver: None,
            call_chain: Vec::new(),
        };
        let generated = crate::verification::testgen::generate_test(
            &witness,
            Some(&path.join("tests/phanalist-generated")),
        )
        .unwrap();
        let mut results = Results::default();
        results.files.insert(
            "src/example.php".to_string(),
            vec![Violation {
                rule: "E0031".to_string(),
                line: "3".to_string(),
                message: crate::results::Message::new("e31", "Division by zero"),
                start_line: 3,
                start_column: 1,
                end_line: 3,
                end_column: 2,
                taint_source: None,
                call_chain: Vec::new(),
                meta: Default::default(),
                generated_test: Some(generated),
            }],
        );
        results
    }

    #[test]
    fn writes_tests_and_patches_project_files_idempotently() {
        let root = temp_root();
        fs::create_dir_all(root.join("src")).unwrap();
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

        let output_dir = root.join("tests/phanalist-generated");
        let summary = write_generated_tests(&mut results_with_test(&root), &output_dir).unwrap();
        assert_eq!(summary.written, 1);
        assert!(summary.composer_updated);
        assert!(summary.phpunit_updated);

        let composer: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("composer.json")).unwrap()).unwrap();
        assert_eq!(
            composer["scripts"][COMPOSER_SCRIPT],
            serde_json::json!("phpunit tests/phanalist-generated")
        );
        let phpunit = fs::read_to_string(root.join("phpunit.xml")).unwrap();
        assert!(phpunit.contains(TESTSUITE_NAME));

        let summary = write_generated_tests(&mut results_with_test(&root), &output_dir).unwrap();
        assert!(!summary.composer_updated);
        assert!(!summary.phpunit_updated);
        fs::remove_dir_all(&root).unwrap();
    }
}
