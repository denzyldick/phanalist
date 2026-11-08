use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::file::File;

use super::summary::TaintSummaryIndex;
use super::taint::{analyze_file, FileTaint, CallGraph, SourceFilter, SinkKind, TaintSummary};

pub(crate) struct SecurityService {
    summaries: Mutex<TaintSummaryIndex>,
    source_filter: SourceFilter,
    /// Call graph accumulated across all indexed files.
    call_graph: Mutex<CallGraph>,
}

impl SecurityService {
    pub fn new() -> Self {
        Self {
            summaries: Mutex::new(TaintSummaryIndex::default()),
            source_filter: SourceFilter::default(),
            call_graph: Mutex::new(CallGraph::new()),
        }
    }

    pub fn with_source_filter(mut self, filter: SourceFilter) -> Self {
        self.source_filter = filter;
        self
    }

    pub fn begin_index(&self) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.clear();
        }
        if let Ok(mut cg) = self.call_graph.lock() {
            *cg = CallGraph::new();
        }
    }

    pub fn index_file(&self, file: &File<'_>) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.replace_file(file, self.source_filter);
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

    pub fn analyze(&self, file: &File<'_>) -> (FileTaint, CallGraph) {
        let snapshot = self
            .summaries
            .lock()
            .ok()
            .map(|summaries| summaries.snapshot());
        let empty = HashMap::<String, TaintSummary>::new();
        let summaries = snapshot.as_deref().unwrap_or(&empty);
        let (taint, cg) = analyze_file(file, summaries, self.source_filter);
        self.merge_call_graph(cg.clone());
        (taint, cg)
    }

    /// Get the accumulated call graph.
    pub fn call_graph(&self) -> CallGraph {
        self.call_graph.lock().unwrap().clone()
    }

    /// Merge call graph edges from analysis into the service.
    pub fn merge_call_graph(&self, cg: CallGraph) {
        if let Ok(mut cg_lock) = self.call_graph.lock() {
            for (caller, callees) in cg.edges {
                for callee in callees {
                    cg_lock.add_edge(&caller, &callee);
                }
            }
        }
    }

    /// Find functions that directly read from request superglobals (sources).
    /// These are functions whose summaries have `returns_source = true`,
    /// meaning they return data that originates from `$_GET`, `$_POST`, etc.
    pub fn find_source_functions(&self) -> Vec<String> {
        let snapshots = self.summaries.lock().unwrap();
        let snap = snapshots.snapshot();
        drop(snapshots);

        snap.iter()
            .filter(|(_, summary)| summary.returns_source)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Find functions that directly pass request-derived parameters to sinks.
    pub fn find_parameter_sink_functions(&self) -> Vec<(String, usize, SinkKind)> {
        let snapshots = self.summaries.lock().unwrap();
        let snap = snapshots.snapshot();
        drop(snapshots);

        let mut results = Vec::new();
        for (name, summary) in snap.iter() {
            for (param_idx, sink_kind) in &summary.param_sinks {
                results.push((name.clone(), *param_idx, *sink_kind));
            }
        }
        results
    }

    /// Get all function names that have summaries (are analyzed).
    pub fn all_function_names(&self) -> Vec<String> {
        let snapshots = self.summaries.lock().unwrap();
        let snap = snapshots.snapshot();
        drop(snapshots);

        snap.keys().cloned().collect()
    }

    /// Check if a function is reachable from any source function via the call graph.
    pub fn is_reachable_from_source(&self, func: &str, source_functions: &[String]) -> bool {
        let cg = self.call_graph.lock().unwrap();
        let mut visited = std::collections::HashSet::new();
        let mut stack: Vec<String> = source_functions.iter().cloned().collect();

        while let Some(current) = stack.pop() {
            if current == func {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(callers) = cg.callers(&current) {
                    for caller in callers {
                        if !visited.contains(caller) {
                            stack.push(caller.clone());
                        }
                    }
                }
            }
        }
        false
    }

    /// Compute the set of functions that are "request-rooted" - functions that either
    /// directly return request data, or call (transitively) a function that does.
    ///
    /// This propagates taint backwards through the call graph: if function B returns
    /// request data, and function A calls B, then A is also tainted because it receives
    /// request data from B's return value.
    pub fn compute_request_rooted_functions(&self) -> std::collections::HashSet<String> {
        let snapshots = self.summaries.lock().unwrap();
        let snap = snapshots.snapshot();
        let cg = self.call_graph.lock().unwrap();
        drop(snapshots);

        // Step 1: Find source functions - functions that directly return request data
        let source_functions: Vec<String> = snap
            .iter()
            .filter(|(_, summary)| summary.returns_source)
            .map(|(name, _)| name.clone())
            .collect();

        // Step 2: BFS backwards through call graph - if function B is tainted,
        // and function A calls B, then A is also tainted
        let mut request_rooted = std::collections::HashSet::new();
        let mut stack: Vec<String> = source_functions.clone();

        while let Some(current) = stack.pop() {
            if request_rooted.insert(current.clone()) {
                if let Some(callers) = cg.callers(&current) {
                    for caller in callers {
                        if !request_rooted.contains(caller) {
                            stack.push(caller.clone());
                        }
                    }
                }
            }
        }

        request_rooted
    }
}
