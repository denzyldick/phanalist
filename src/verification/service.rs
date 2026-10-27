use std::path::Path;
use std::sync::Mutex;

use crate::file::File;

use super::class::FindingClass;
use super::engine::{self, Finding};
use super::summary::SummaryIndex;
use super::Settings;

#[derive(Default)]
pub struct VerificationService {
    summaries: Mutex<SummaryIndex>,
}

impl VerificationService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_index(&self) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.clear();
        }
    }

    pub fn index_file(&self, file: &File<'_>) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.replace_file(file);
        }
    }

    pub fn finalize_index(&self) {}

    pub fn remove_file(&self, path: &Path) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.remove_file(path);
        }
    }

    pub fn analyze(
        &self,
        file: &File<'_>,
        settings: &Settings,
        classes: &[FindingClass],
    ) -> Vec<Finding> {
        let (summaries, parents) = self
            .summaries
            .lock()
            .map(|summaries| (summaries.snapshot(), summaries.parent_map()))
            .unwrap_or_default();
        engine::analyze_file_with_summaries(file, settings, &summaries, &parents, classes)
    }
}
