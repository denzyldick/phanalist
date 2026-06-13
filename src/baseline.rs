//! Baseline support: freeze the current set of violations and, on later runs,
//! report only new ones. Keyed on `(path, file, rule, message id)` with a count,
//! so line shifts and reworded message text do not invalidate it.
//!
//! NOTE: `normalize_relative` / `split_dir_file` here intentionally mirror the
//! path helpers introduced by the `exclude_paths` work. When both land they can
//! be unified into a single shared module.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::results::Results;

const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaselineEntry {
    pub path: String,
    pub file: String,
    pub rule: String,
    pub id: String,
    /// Message template, carried for human readability only. Not part of the key.
    pub message: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Baseline {
    pub version: u32,
    pub violations: Vec<BaselineEntry>,
}

/// Normalize a path string to a stable, portable relative form: relative to the
/// current working directory, posix separators, no leading `./`.
pub fn normalize_relative(path: &str) -> String {
    let path = path.replace('\\', "/");
    let path = match std::env::current_dir() {
        Ok(cwd) => {
            let cwd = cwd.to_string_lossy().replace('\\', "/");
            let prefix = format!("{}/", cwd.trim_end_matches('/'));
            path.strip_prefix(&prefix).map(String::from).unwrap_or(path)
        }
        Err(_) => path,
    };
    path.strip_prefix("./").unwrap_or(&path).to_string()
}

/// Split a normalized relative path into its directory portion and file name.
/// A path with no directory yields an empty directory string.
pub fn split_dir_file(relative: &str) -> (String, String) {
    match relative.rsplit_once('/') {
        Some((dir, file)) => (dir.to_string(), file.to_string()),
        None => (String::new(), relative.to_string()),
    }
}

impl Baseline {
    /// Build a baseline by grouping a run's violations on
    /// `(path, file, rule, id)` and counting each group. Entries are sorted so
    /// the serialized file is deterministic.
    pub fn from_results(results: &Results) -> Baseline {
        // key -> (template, count)
        let mut groups: HashMap<(String, String, String, String), (String, usize)> =
            HashMap::new();

        for (path, violations) in &results.files {
            let (dir, file) = split_dir_file(&normalize_relative(path));
            for violation in violations {
                let key = (
                    dir.clone(),
                    file.clone(),
                    violation.rule.clone(),
                    violation.message.id.clone(),
                );
                let entry = groups
                    .entry(key)
                    .or_insert_with(|| (violation.message.template.clone(), 0));
                entry.1 += 1;
            }
        }

        let mut violations: Vec<BaselineEntry> = groups
            .into_iter()
            .map(|((path, file, rule, id), (message, count))| BaselineEntry {
                path,
                file,
                rule,
                id,
                message,
                count,
            })
            .collect();

        violations.sort_by(|a, b| {
            (&a.path, &a.file, &a.rule, &a.id).cmp(&(&b.path, &b.file, &b.rule, &b.id))
        });

        Baseline {
            version: VERSION,
            violations,
        }
    }

    pub fn to_pretty_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("baseline serializes")
    }

    pub fn from_json(text: &str) -> serde_json::Result<Baseline> {
        serde_json::from_str(text)
    }

    pub fn load(path: &Path) -> std::io::Result<Baseline> {
        let text = std::fs::read_to_string(path)?;
        Self::from_json(&text)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &PathBuf) -> std::io::Result<()> {
        std::fs::write(path, self.to_pretty_json())
    }

    /// Drop from `results` every violation already accounted for in the
    /// baseline, leaving only the surplus (new) ones, and recompute counts.
    pub fn filter(&self, results: &mut Results) {
        // key -> baseline budget (how many of this key are already known)
        let budget: HashMap<(&str, &str, &str, &str), usize> = self
            .violations
            .iter()
            .map(|e| {
                (
                    (
                        e.path.as_str(),
                        e.file.as_str(),
                        e.rule.as_str(),
                        e.id.as_str(),
                    ),
                    e.count,
                )
            })
            .collect();

        for (path, violations) in results.files.iter_mut() {
            let (dir, file) = split_dir_file(&normalize_relative(path));
            // How many of each key we have suppressed so far in this file.
            let mut seen: HashMap<(String, String), usize> = HashMap::new();

            violations.retain(|violation| {
                let key = (violation.rule.clone(), violation.message.id.clone());
                let count = seen.entry(key).or_insert(0);
                *count += 1;
                let allowed = budget
                    .get(&(
                        dir.as_str(),
                        file.as_str(),
                        violation.rule.as_str(),
                        violation.message.id.as_str(),
                    ))
                    .copied()
                    .unwrap_or(0);
                // Keep only the surplus beyond the baselined count.
                *count > allowed
            });
        }

        // Recompute counts from what survived.
        let mut codes_count: HashMap<String, i64> = HashMap::new();
        for violations in results.files.values() {
            for violation in violations {
                *codes_count.entry(violation.rule.clone()).or_insert(0) += 1;
            }
        }
        results.codes_count = codes_count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{Message, Violation};

    fn vio(rule: &str, id: &str, template: &str) -> Violation {
        Violation {
            rule: rule.to_string(),
            line: String::new(),
            message: Message::new(id, template),
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
        }
    }

    fn results_with(files: Vec<(&str, Vec<Violation>)>) -> Results {
        let mut r = Results::default();
        for (path, violations) in files {
            for v in &violations {
                *r.codes_count.entry(v.rule.clone()).or_insert(0) += 1;
            }
            r.files.insert(path.to_string(), violations);
        }
        r
    }

    #[test]
    fn normalize_strips_dot_slash_and_backslashes() {
        assert_eq!(normalize_relative("./src/Foo.php"), "src/Foo.php");
        assert_eq!(normalize_relative("src\\Foo.php"), "src/Foo.php");
        assert_eq!(normalize_relative("src/Foo.php"), "src/Foo.php");
    }

    #[test]
    fn split_dir_file_separates_directory_and_name() {
        assert_eq!(
            split_dir_file("src/Service/Foo.php"),
            ("src/Service".to_string(), "Foo.php".to_string())
        );
        assert_eq!(
            split_dir_file("Foo.php"),
            (String::new(), "Foo.php".to_string())
        );
    }

    #[test]
    fn from_results_groups_and_counts_by_key() {
        let results = results_with(vec![(
            "./src/Service/Foo.php",
            vec![
                vio("E0009", "E0009:complexity", "msg a"),
                vio("E0009", "E0009:complexity", "msg b"),
                vio("E0005", "E0005:name", "msg c"),
            ],
        )]);

        let baseline = Baseline::from_results(&results);

        assert_eq!(baseline.version, VERSION);
        assert_eq!(baseline.violations.len(), 2);
        let complexity = baseline
            .violations
            .iter()
            .find(|e| e.id == "E0009:complexity")
            .unwrap();
        assert_eq!(complexity.path, "src/Service");
        assert_eq!(complexity.file, "Foo.php");
        assert_eq!(complexity.count, 2);
    }

    #[test]
    fn from_results_sorts_entries() {
        let results = results_with(vec![
            ("./b/Z.php", vec![vio("E0002", "E0002:x", "m")]),
            ("./a/Y.php", vec![vio("E0001", "E0001:x", "m")]),
        ]);
        let baseline = Baseline::from_results(&results);
        let keys: Vec<&str> = baseline.violations.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn json_round_trips_and_is_deterministic() {
        let results = results_with(vec![(
            "./src/Foo.php",
            vec![vio("E0009", "E0009:complexity", "msg")],
        )]);
        let baseline = Baseline::from_results(&results);

        let json = baseline.to_pretty_json();
        assert_eq!(baseline.to_pretty_json(), json, "serialization must be stable");
        assert_eq!(Baseline::from_json(&json).unwrap(), baseline);
    }

    #[test]
    fn save_then_load_round_trips() {
        let results = results_with(vec![(
            "./src/Foo.php",
            vec![vio("E0009", "E0009:complexity", "msg")],
        )]);
        let baseline = Baseline::from_results(&results);

        let path =
            std::env::temp_dir().join(format!("phanalist_bl_{}.json", std::process::id()));
        baseline.save(&path).unwrap();
        let loaded = Baseline::load(&path).unwrap();
        std::fs::remove_file(&path).ok();

        assert_eq!(loaded, baseline);
    }

    #[test]
    fn filter_suppresses_when_count_equal() {
        let mut results = results_with(vec![(
            "./src/Foo.php",
            vec![vio("E0009", "E0009:c", "m")],
        )]);
        let baseline = Baseline::from_results(&results);

        baseline.filter(&mut results);

        assert!(!results.has_any_violations());
        assert!(results.files.get("./src/Foo.php").unwrap().is_empty());
    }

    #[test]
    fn filter_reports_surplus_when_count_exceeds_baseline() {
        let baseline_results =
            results_with(vec![("./src/Foo.php", vec![vio("E0009", "E0009:c", "m")])]);
        let baseline = Baseline::from_results(&baseline_results);

        // Now the same file has two of that violation: one is baselined, one is new.
        let mut results = results_with(vec![(
            "./src/Foo.php",
            vec![vio("E0009", "E0009:c", "m"), vio("E0009", "E0009:c", "m")],
        )]);

        baseline.filter(&mut results);

        assert_eq!(results.files.get("./src/Foo.php").unwrap().len(), 1);
        assert_eq!(*results.codes_count.get("E0009").unwrap(), 1);
    }

    #[test]
    fn filter_keeps_violations_absent_from_baseline() {
        let baseline = Baseline {
            version: VERSION,
            violations: vec![],
        };
        let mut results =
            results_with(vec![("./src/Foo.php", vec![vio("E0009", "E0009:c", "m")])]);

        baseline.filter(&mut results);

        assert_eq!(results.files.get("./src/Foo.php").unwrap().len(), 1);
        assert_eq!(*results.codes_count.get("E0009").unwrap(), 1);
    }
}
