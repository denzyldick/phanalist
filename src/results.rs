use std::collections::HashMap;
use std::time::Duration;

use php_parser_rs::lexer::token::Span;
use serde::{Deserialize, Serialize};

use crate::file::File;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
