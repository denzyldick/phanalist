use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::file::File;

use super::taint::{summarize_file, SourceFilter, TaintSummary};

#[derive(Default)]
pub(crate) struct TaintSummaryIndex {
    files: HashMap<PathBuf, Vec<(String, TaintSummary)>>,
    snapshot_cache: Mutex<Option<Arc<HashMap<String, TaintSummary>>>>,
    source_filter: SourceFilter,
}

impl TaintSummaryIndex {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            snapshot_cache: Mutex::new(None),
            source_filter: SourceFilter::default(),
        }
    }

    pub fn with_source_filter(mut self, filter: SourceFilter) -> Self {
        self.source_filter = filter;
        self
    }

    pub fn clear(&mut self) {
        self.files.clear();
        self.invalidate();
    }

    pub fn remove_file(&mut self, path: &Path) {
        self.files.remove(path);
        self.invalidate();
    }

    pub fn replace_file(&mut self, file: &File<'_>, source_filter: SourceFilter) {
        self.files.remove(&file.path);
        self.invalidate();
        let summaries = summarize_file(file, source_filter);
        if !summaries.is_empty() {
            self.files.insert(file.path.clone(), summaries);
        }
    }

    pub fn snapshot(&self) -> Arc<HashMap<String, TaintSummary>> {
        if let Some(snapshot) = self
            .snapshot_cache
            .lock()
            .ok()
            .and_then(|cache| cache.clone())
        {
            return snapshot;
        }

        let mut counts: HashMap<String, usize> = HashMap::new();
        for (name, _) in self.files.values().flat_map(|entries| entries.iter()) {
            *counts.entry(name.clone()).or_default() += 1;
        }

        let mut summaries = HashMap::new();
        for (name, summary) in self.files.values().flat_map(|entries| entries.iter()) {
            if counts.get(name) == Some(&1) {
                summaries.insert(name.clone(), summary.clone());
            }
        }

        let snapshot = Arc::new(summaries);
        if let Ok(mut cache) = self.snapshot_cache.lock() {
            *cache = Some(snapshot.clone());
        }
        snapshot
    }

    fn invalidate(&self) {
        if let Ok(mut cache) = self.snapshot_cache.lock() {
            *cache = None;
        }
    }
}
