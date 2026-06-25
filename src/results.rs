use std::collections::HashMap;
use std::time::Duration;

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use crate::debug_stats::RuleTimings;
use crate::file::File;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EngineerEntry {
    pub total_fixed: u64,
    pub total_introduced: u64,
    pub net: i64,
    pub rules: HashMap<String, RuleChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleChange {
    pub fixed: u64,
    pub introduced: u64,
}

pub type EngineerReport = HashMap<String, EngineerEntry>;

/// A diagnostic message. `id` is a stable slug used as a key (e.g. by the
/// baseline); `template` is human text with `{name}` placeholders that `args`
/// fill in. `render()` produces the displayed string. Marked `#[non_exhaustive]`
/// so future fields (severity, help url, fix data) are additive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub struct Message {
    pub id: String,
    pub template: String,
    pub args: Vec<(String, String)>,
}

impl Message {
    pub fn new(id: impl Into<String>, template: impl Into<String>) -> Self {
        Message {
            id: id.into(),
            template: template.into(),
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.args.push((name.into(), value.into()));
        self
    }

    /// Substitute `args` into the `{name}` placeholders of the template.
    pub fn render(&self) -> String {
        let mut out = self.template.clone();
        for (name, value) in &self.args {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Violation {
    pub rule: String,
    pub line: String,
    pub message: Message,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

// Custom serialization: emit the structured `message` and also a flat, rendered
// `suggestion` string so consumers of the JSON output that read the old
// `suggestion` field keep working.
impl Serialize for Violation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Violation", 8)?;
        state.serialize_field("rule", &self.rule)?;
        state.serialize_field("line", &self.line)?;
        state.serialize_field("suggestion", &self.message.render())?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("start_line", &self.start_line)?;
        state.serialize_field("start_column", &self.start_column)?;
        state.serialize_field("end_line", &self.end_line)?;
        state.serialize_field("end_column", &self.end_column)?;
        state.end()
    }
}

#[derive(Serialize, Debug, Deserialize, Clone, Default)]
pub struct Results {
    pub files: HashMap<String, Vec<Violation>>,
    pub codes_count: HashMap<String, i64>,
    pub total_files_count: i64,
    pub duration: Option<Duration>,
    #[serde(skip)]
    pub rule_timings: Option<RuleTimings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engineer_report: Option<EngineerReport>,
}

impl Results {
    pub fn add_file_violations(&mut self, file: &File<'_>, violations: Vec<Violation>) {
        let path = file.path.display().to_string();

        let mut current_file_violations = if let Some(s) = self.files.get(&path) {
            s.to_owned()
        } else {
            vec![]
        };

        for violation in violations {
            current_file_violations.push(violation.clone());

            let mut rule_count = if let Some(count) = self.codes_count.get(&violation.rule) {
                count.to_owned()
            } else {
                0
            };
            rule_count += 1;

            self.codes_count.insert(violation.rule, rule_count);
        }

        self.files.insert(path, current_file_violations);
    }

    pub fn has_any_violations(&self) -> bool {
        self.codes_count.values().any(|&c| c > 0)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use mago_allocator::prelude::LocalArena;

    use super::*;

    fn get_results() -> Results {
        Results {
            files: Default::default(),
            codes_count: Default::default(),
            total_files_count: 0,
            duration: None,
            rule_timings: None,
            engineer_report: None,
        }
    }
    fn get_file<'a>(arena: &'a LocalArena, name: &str) -> File<'a> {
        File::new(arena, PathBuf::from(name), "Content".to_string())
    }
    fn get_violation(rule: &str) -> Violation {
        Violation {
            rule: rule.to_string(),
            line: "Line".to_string(),
            message: Message::new("test", "Suggestion"),
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
        }
    }

    #[test]
    fn message_render_without_args_returns_template() {
        let m = Message::new("e1.x", "A plain message.");
        assert_eq!(m.render(), "A plain message.");
    }

    #[test]
    fn message_render_substitutes_named_args() {
        let m = Message::new("e1.col", "Wrong column: {column}.").arg("column", "2");
        assert_eq!(m.render(), "Wrong column: 2.");
    }

    #[test]
    fn message_render_substitutes_multiple_args() {
        let m = Message::new("e", "{a} then {b}")
            .arg("a", "first")
            .arg("b", "second");
        assert_eq!(m.render(), "first then second");
    }

    #[test]
    fn message_keeps_id() {
        assert_eq!(Message::new("e1.x", "t").id, "e1.x");
    }

    #[test]
    fn violation_json_has_rendered_suggestion_and_structured_message() {
        let v = get_violation("E001");
        let json = serde_json::to_string(&v).unwrap();
        // Backward-compatible flat rendered string...
        assert!(json.contains("\"suggestion\":\"Suggestion\""));
        // ...plus the structured message with its stable id.
        assert!(json.contains("\"message\":{"));
        assert!(json.contains("\"id\":\"test\""));
    }

    #[test]
    fn test_add_file_violations_expected_file_violations() {
        let mut results = get_results();

        let arena = LocalArena::new();
        let file1 = get_file(&arena, "./class1.php");
        let file2 = get_file(&arena, "./class2.php");

        let violation1 = get_violation("E001");
        let violation2 = get_violation("E002");
        let violation3 = get_violation("E003");
        let violation4 = get_violation("E004");

        results.add_file_violations(&file1, vec![violation1.clone(), violation2.clone()]);
        results.add_file_violations(&file2, vec![violation3.clone()]);
        results.add_file_violations(&file1, vec![violation4.clone()]);

        let expected_file1_violations = vec![violation1, violation2, violation4];
        let expected_file2_violations = vec![violation3];

        assert_eq!(
            results.files.get("./class1.php").unwrap(),
            &expected_file1_violations
        );
        assert_eq!(
            results.files.get("./class2.php").unwrap(),
            &expected_file2_violations
        );
    }

    #[test]
    fn test_add_file_violations_expected_codes_count() {
        let mut results = get_results();

        let arena = LocalArena::new();
        let file1 = get_file(&arena, "./class1.php");
        let file2 = get_file(&arena, "./class2.php");

        let violation1 = get_violation("E001");
        let violation2 = get_violation("E002");
        let violation3 = get_violation("E003");
        let violation4 = get_violation("E001");

        results.add_file_violations(&file1, vec![violation1, violation2]);
        results.add_file_violations(&file2, vec![violation3]);
        results.add_file_violations(&file1, vec![violation4]);

        let mut expected_codes_count: HashMap<String, i64> = HashMap::new();
        expected_codes_count.insert("E001".to_string(), 2);
        expected_codes_count.insert("E002".to_string(), 1);
        expected_codes_count.insert("E003".to_string(), 1);

        assert_eq!(results.codes_count, expected_codes_count);
    }

    #[test]
    fn test_has_any_violations_expected_true() {
        let mut results = get_results();
        let arena = LocalArena::new();
        let file1 = get_file(&arena, "./class1.php");
        let violation1 = get_violation("E001");

        results.add_file_violations(&file1, vec![violation1]);

        assert!(results.has_any_violations());
    }

    #[test]
    fn test_has_any_violations_expected_false() {
        let results = get_results();
        assert!(!results.has_any_violations());
    }
}
