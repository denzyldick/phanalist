use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::file::File;

use super::summary::TaintSummaryIndex;
use super::taint::{analyze_file, FileTaint, TaintSummary};

pub(crate) struct SecurityService {
    summaries: Mutex<TaintSummaryIndex>,
}

impl SecurityService {
    pub fn new() -> Self {
        Self {
            summaries: Mutex::new(TaintSummaryIndex::default()),
        }
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

    pub fn finalize_index(&self) {
        if let Ok(summaries) = self.summaries.lock() {
            let _ = summaries.snapshot();
        }
    }

    pub fn remove_file(&self, path: &Path) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.remove_file(path);
        }
    }

    pub fn analyze(&self, file: &File<'_>) -> FileTaint {
        let snapshot = self
            .summaries
            .lock()
            .ok()
            .map(|summaries| summaries.snapshot());
        let empty = HashMap::<String, TaintSummary>::new();
        let summaries = snapshot.as_deref().unwrap_or(&empty);
        analyze_file(file, summaries)
    }
}
