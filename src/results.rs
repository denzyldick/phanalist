use std::collections::HashMap;
use std::time::Duration;

use php_parser_rs::lexer::token::Span;
use serde::{Deserialize, Serialize};

use crate::file::File;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Violation {
    pub rule: String,
    pub line: String,
    pub suggestion: String,
    pub span: Span,
}

#[derive(Serialize, Debug, Deserialize, Clone, Default)]
pub struct Results {
    pub files: HashMap<String, Vec<Violation>>,
    pub codes_count: HashMap<String, i64>,
    pub total_files_count: i64,
    pub duration: Option<Duration>,
}

impl Results {
    pub fn add_file_violations(&mut self, file: &File, violations: Vec<Violation>) {
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
        self.codes_count
            .values()
            .cloned()
            .collect::<Vec<i64>>()
            .iter()
            .sum::<i64>()
            > 0
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn get_results() -> Results {
        Results {
            files: Default::default(),
            codes_count: Default::default(),
            total_files_count: 0,
            duration: None,
        }
    }
    fn get_file(name: &str) -> File {
        File::new(PathBuf::from(name), "Content".to_string())
    }
    fn get_violation(rule: &str) -> Violation {
        Violation {
            rule: rule.to_string(),
            line: "Line".to_string(),
            suggestion: "Suggestion".to_string(),
            span: Span {
                line: 0,
                column: 0,
                position: 0,
            },
        }
    }

    #[test]
    fn test_add_file_violations_expected_file_violations() {
        let mut results = get_results();

        let file1 = get_file("./class1.php");
        let file2 = get_file("./class2.php");

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

        let file1 = get_file("./class1.php");
        let file2 = get_file("./class2.php");

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
        let file1 = get_file("./class1.php");
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
