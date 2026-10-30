use std::path::Path;
use std::sync::Mutex;

use crate::file::File;

use super::analyzer::{self, ResourceClass, ResourceFinding, ResourceSettings};
use super::summary::ResourceSummaryIndex;

#[derive(Default)]
pub struct ResourceService {
    summaries: Mutex<ResourceSummaryIndex>,
}

impl ResourceService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_index(&self) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.clear();
        }
    }

    pub fn index_file(&self, file: &File<'_>) {
        let mut local = ResourceSummaryIndex::default();
        local.replace_file(file);
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.merge(local);
        }
    }

    pub fn finalize_index(&self) {
        if let Ok(summaries) = self.summaries.lock() {
            // Warm the project-wide cache once before parallel validation.
            let _ = summaries.snapshot();
        }
    }

    pub fn remove_file(&self, path: &Path) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.remove_file(path);
        }
    }

    pub fn analyze(
        &self,
        file: &File<'_>,
        settings: &ResourceSettings,
        classes: &[ResourceClass],
    ) -> Vec<ResourceFinding> {
        let summaries = self
            .summaries
            .lock()
            .map(|summaries| summaries.snapshot())
            .unwrap_or_default();
        analyzer::analyze(file, settings, classes, &summaries)
    }
}
