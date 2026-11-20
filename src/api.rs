//! UI/embedding-friendly API facade for running Phanalist scans.
//!
//! This module provides progress and cancellation hooks and avoids CLI concerns
//! like printing or exiting the process.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use mago_allocator::prelude::LocalArena;

use crate::analyse::{collect_php_files, read_source, scan_worker_count, Analyse};
use crate::config::{Config, ConfigLoadInfo};
use crate::file::File;

use crate::results::{Results, Violation};

/// Phase of a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    Preparing,
    Discovering,
    Indexing,
    Analyzing,
    Finalizing,
    Done,
    Cancelled,
}

/// Progress update emitted during a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProgress {
    pub phase: ScanPhase,
    pub files_done: usize,
    pub files_total: usize,
    pub current_path: Option<String>,
    pub message: Option<String>,
}

/// Cooperative cancellation token.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    pub fn raw(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }
}

/// Request for a scan.
#[derive(Debug, Clone)]
pub struct ScanRequest {
    /// Source paths/directories to scan.
    pub paths: Vec<PathBuf>,
    /// Configuration to use. If `None`, `config_path` is loaded/created.
    pub config: Option<Config>,
    /// Path to `phanalist.yaml` (used when `config` is `None`, and for defaults context).
    pub config_path: Option<PathBuf>,
    /// Override taint source behavior (true = all sources/leads, false = request-only).
    pub taint_source_all: Option<bool>,
    /// Collect per-rule/file timing diagnostics (additive, opt-in).
    pub collect_metrics: bool,
}

impl Default for ScanRequest {
    fn default() -> Self {
        Self {
            paths: vec![PathBuf::from("./src")],
            config: None,
            config_path: Some(PathBuf::from("./phanalist.yaml")),
            taint_source_all: None,
            collect_metrics: false,
        }
    }
}

/// Result of preparing/loading config for a scan.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: Config,
    pub info: Option<ConfigLoadInfo>,
    pub path: Option<PathBuf>,
}

/// Errors returned by the GUI/embedding API.
#[derive(Debug, Clone)]
pub enum ScanError {
    InvalidPath(String),
    Io(String),
    Config(String),
    Cancelled,
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::InvalidPath(p) => write!(f, "Path does not exist: {p}"),
            ScanError::Io(e) => write!(f, "IO error: {e}"),
            ScanError::Config(e) => write!(f, "Config error: {e}"),
            ScanError::Cancelled => write!(f, "Scan cancelled"),
        }
    }
}

impl std::error::Error for ScanError {}

/// Resolve/load configuration for a request.
pub fn load_config(request: &ScanRequest) -> Result<LoadedConfig, ScanError> {
    if let Some(cfg) = &request.config {
        let mut cfg = cfg.clone();
        if let Some(v) = request.taint_source_all {
            cfg.taint_source_all = v;
        }
        return Ok(LoadedConfig {
            config: cfg,
            info: None,
            path: request.config_path.clone(),
        });
    }

    let path = request
        .config_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("./phanalist.yaml"));
    let (mut cfg, info) =
        Config::load_or_create(&path).map_err(|e| ScanError::Io(e.to_string()))?;
    if let Some(v) = request.taint_source_all {
        cfg.taint_source_all = v;
    }
    Ok(LoadedConfig {
        config: cfg,
        info: Some(info),
        path: Some(path),
    })
}

fn is_cancelled(cancel: Option<&CancelToken>) -> bool {
    cancel.map(|c| c.is_cancelled()).unwrap_or(false)
}

/// Run a scan and return aggregated results with fine-grained progress.
///
/// Uses parallel indexing/analysis where possible (similar to the CLI engine) while
/// emitting progress updates from the main scan thread as work completes.
pub fn scan<F>(
    request: ScanRequest,
    cancel: Option<&CancelToken>,
    mut on_progress: Option<F>,
) -> Result<(Results, LoadedConfig), ScanError>
where
    F: FnMut(ScanProgress),
{
    let loaded = load_config(&request)?;
    let mut emit = |p: ScanProgress| {
        if let Some(cb) = on_progress.as_mut() {
            cb(p);
        }
    };

    emit(ScanProgress {
        phase: ScanPhase::Preparing,
        files_done: 0,
        files_total: 0,
        current_path: None,
        message: Some("Preparing scan".to_string()),
    });

    for path in &request.paths {
        if !path.exists() {
            return Err(ScanError::InvalidPath(path.display().to_string()));
        }
    }
    if is_cancelled(cancel) {
        return Err(ScanError::Cancelled);
    }

    // 1) Discover files
    emit(ScanProgress {
        phase: ScanPhase::Discovering,
        files_done: 0,
        files_total: 0,
        current_path: None,
        message: Some("Discovering PHP files...".to_string()),
    });

    let mut seen = HashSet::new();
    let mut all_files: Vec<PathBuf> = Vec::new();
    for (ri, root) in request.paths.iter().enumerate() {
        emit(ScanProgress {
            phase: ScanPhase::Discovering,
            files_done: all_files.len(),
            files_total: 0,
            current_path: Some(root.display().to_string()),
            message: Some(format!(
                "Scanning source root {}/{}: {}",
                ri + 1,
                request.paths.len(),
                root.display()
            )),
        });
        let files = collect_php_files(root.clone(), 0, None, &loaded.config.exclude_paths);
        for file in files {
            let key = crate::paths::normalize_relative(&file);
            if seen.insert(key) {
                all_files.push(file.clone());
                emit(ScanProgress {
                    phase: ScanPhase::Discovering,
                    files_done: all_files.len(),
                    files_total: 0,
                    current_path: Some(file.display().to_string()),
                    message: Some(format!("Discovered {} PHP files...", all_files.len())),
                });
                if is_cancelled(cancel) {
                    return Err(ScanError::Cancelled);
                }
            }
        }
    }
    let total = all_files.len();
    emit(ScanProgress {
        phase: ScanPhase::Discovering,
        files_done: total,
        files_total: total,
        current_path: None,
        message: Some(format!("Discovery complete: {} PHP files", total)),
    });

    // 2) Index (parallel service indexing + serial rule indexing for safety/parity)
    emit(ScanProgress {
        phase: ScanPhase::Indexing,
        files_done: 0,
        files_total: total,
        current_path: None,
        message: Some("Initializing analysis/index state...".to_string()),
    });
    let analyze = Analyse::new(&loaded.config);
    analyze.begin_index();

    let worker_count = scan_worker_count(total);

    // 2a) Parallel service indexing (verification/resource/security), when applicable.
    if worker_count > 1
        && (analyze.has_verification_rules()
            || analyze.has_resource_rules()
            || analyze.has_security_rules())
    {
        emit(ScanProgress {
            phase: ScanPhase::Indexing,
            files_done: 0,
            files_total: total,
            current_path: None,
            message: Some(format!(
                "Indexing services in parallel ({} workers)...",
                worker_count
            )),
        });

        let next = AtomicUsize::new(0);
        let done = AtomicUsize::new(0);
        let (tx, rx) = mpsc::channel::<(usize, String)>();

        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let next = &next;
                let done = &done;
                let files = &all_files;
                let analyze = &analyze;
                let tx = tx.clone();
                scope.spawn(move || {
                    let mut arena = LocalArena::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(file_path) = files.get(index) else {
                            break;
                        };
                        if let Some(content) = read_source(file_path) {
                            {
                                let file = File::new(&arena, file_path.clone(), content);
                                analyze.index_service_file(&file);
                            }
                            arena.reset();
                        }
                        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                        let _ = tx.send((d, file_path.display().to_string()));
                    }
                });
            }
            drop(tx);

            for (d, path) in rx {
                if is_cancelled(cancel) {
                    // best-effort: continue draining to let workers finish
                    continue;
                }
                emit(ScanProgress {
                    phase: ScanPhase::Indexing,
                    files_done: d.min(total),
                    files_total: total,
                    current_path: Some(path.clone()),
                    message: Some(format!(
                        "Indexing services {}/{}: {}",
                        d.min(total),
                        total,
                        path
                    )),
                });
            }
        });

        if is_cancelled(cancel) {
            return Err(ScanError::Cancelled);
        }
    }

    // 2b) Rule indexing (serial for parity/safety) with per-file progress.
    emit(ScanProgress {
        phase: ScanPhase::Indexing,
        files_done: 0,
        files_total: total,
        current_path: None,
        message: Some("Indexing rule state...".to_string()),
    });
    let mut arena = LocalArena::new();
    for (i, file_path) in all_files.iter().enumerate() {
        if is_cancelled(cancel) {
            return Err(ScanError::Cancelled);
        }
        emit(ScanProgress {
            phase: ScanPhase::Indexing,
            files_done: i,
            files_total: total,
            current_path: Some(file_path.display().to_string()),
            message: Some(format!(
                "Indexing rules {}/{}: {}",
                i + 1,
                total,
                file_path.display()
            )),
        });

        let Some(content) = read_source(file_path) else {
            continue;
        };
        {
            let file = File::new(&arena, file_path.clone(), content);
            analyze.index_rule_file(&file);
        }
        arena.reset();
    }

    analyze.finalize_index_with_progress(|step| {
        emit(ScanProgress {
            phase: ScanPhase::Indexing,
            files_done: total,
            files_total: total,
            current_path: None,
            message: Some(step.to_string()),
        });
    });

    if is_cancelled(cancel) {
        return Err(ScanError::Cancelled);
    }

    emit(ScanProgress {
        phase: ScanPhase::Analyzing,
        files_done: 0,
        files_total: total,
        current_path: None,
        message: Some(format!("Starting analysis of {} files...", total)),
    });

    // 3) Analyze (parallel when possible), emitting progress as outcomes arrive.
    let now = Instant::now();
    let mut results = Results::default();
    if request.collect_metrics {
        results.rule_timings = Some(crate::debug_stats::RuleTimings::default());
    }

    struct FileOutcome {
        path: PathBuf,
        violations: Vec<Violation>,
        timings: Option<crate::debug_stats::FileTimings>,
    }

    if worker_count <= 1 || total <= 1 {
        let mut arena = LocalArena::new();
        for (i, file_path) in all_files.iter().enumerate() {
            if is_cancelled(cancel) {
                return Err(ScanError::Cancelled);
            }
            emit(ScanProgress {
                phase: ScanPhase::Analyzing,
                files_done: i,
                files_total: total,
                current_path: Some(file_path.display().to_string()),
                message: Some(format!(
                    "Analyzing {}/{}: {}",
                    i + 1,
                    total,
                    file_path.display()
                )),
            });

            let Some(content) = read_source(file_path) else {
                continue;
            };

            {
                let mut file = File::new(&arena, file_path.clone(), content);
                let (violations, timings) =
                    analyze.analyse_file(&mut file, request.collect_metrics);
                results.add_file_violations(&file, violations);
                if let (Some(rt), Some(ft), Some(path_str)) = (
                    results.rule_timings.as_mut(),
                    timings,
                    Some(file_path.display().to_string()),
                ) {
                    rt.merge_file(path_str, ft);
                }
            }
            arena.reset();
        }
    } else {
        let next = AtomicUsize::new(0);
        let (tx, rx) = mpsc::channel::<FileOutcome>();
        std::thread::scope(|scope| {
            for _ in 0..worker_count {
                let tx = tx.clone();
                let next = &next;
                let files = &all_files;
                let analyze = &analyze;
                scope.spawn(move || {
                    let mut arena = LocalArena::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(file_path) = files.get(index) else {
                            break;
                        };
                        let Some(content) = read_source(file_path) else {
                            continue;
                        };

                        let outcome = {
                            let mut file = File::new(&arena, file_path.clone(), content);
                            let (violations, timings) =
                                analyze.analyse_file(&mut file, request.collect_metrics);
                            FileOutcome {
                                path: file.path.clone(),
                                violations,
                                timings,
                            }
                        };
                        arena.reset();

                        if tx.send(outcome).is_err() {
                            break;
                        }
                    }
                });
            }
            drop(tx);

            let mut done = 0usize;
            for outcome in rx {
                done += 1;
                if !is_cancelled(cancel) {
                    emit(ScanProgress {
                        phase: ScanPhase::Analyzing,
                        files_done: done.saturating_sub(1),
                        files_total: total,
                        current_path: Some(outcome.path.display().to_string()),
                        message: Some(format!(
                            "Analyzing {}/{}: {}",
                            done.min(total),
                            total,
                            outcome.path.display()
                        )),
                    });
                }
                let path_str = outcome.path.display().to_string();
                results.add_file_violations_for_path(outcome.path, outcome.violations);
                if let (Some(rt), Some(ft)) = (results.rule_timings.as_mut(), outcome.timings) {
                    rt.merge_file(path_str, ft);
                }
            }
        });

        if is_cancelled(cancel) {
            return Err(ScanError::Cancelled);
        }
    }

    // 4) Post-processing taint tiering (mirror scan_prepared behavior)
    emit(ScanProgress {
        phase: ScanPhase::Finalizing,
        files_done: total,
        files_total: total,
        current_path: None,
        message: Some("Applying taint/result tiering...".to_string()),
    });

    analyze.apply_taint_tiering(&mut results);

    emit(ScanProgress {
        phase: ScanPhase::Finalizing,
        files_done: total,
        files_total: total,
        current_path: None,
        message: Some("Computing summary and finalizing...".to_string()),
    });

    results.total_files_count = total as i64;
    results.duration = Some(now.elapsed());

    if is_cancelled(cancel) {
        return Err(ScanError::Cancelled);
    }

    emit(ScanProgress {
        phase: ScanPhase::Done,
        files_done: total,
        files_total: total,
        current_path: None,
        message: Some("Scan complete".to_string()),
    });

    Ok((results, loaded))
}

/// Return the effective rule settings for a config (all rules, reflecting overrides).
pub fn effective_rule_settings(
    config: &Config,
) -> std::collections::HashMap<String, serde_json::Value> {
    let analyze = Analyse::new(config);
    analyze.effective_rule_settings()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_token_starts_inactive_and_can_be_cancelled() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());

        token.cancel();
        assert!(token.is_cancelled());
        assert!(token.raw().load(Ordering::Relaxed));
    }

    #[test]
    fn cancel_token_clones_share_the_same_flag() {
        let token = CancelToken::new();
        let clone = token.clone();

        token.cancel();

        assert!(clone.is_cancelled());
    }

    #[test]
    fn scan_request_default_targets_src_and_phanalist_yaml() {
        let request = ScanRequest::default();

        assert_eq!(request.paths, vec![PathBuf::from("./src")]);
        assert_eq!(request.config_path, Some(PathBuf::from("./phanalist.yaml")));
        assert!(request.config.is_none());
        assert!(!request.collect_metrics);
    }

    #[test]
    fn load_config_uses_the_provided_config_and_applies_taint_override() {
        let mut config = Config::default();
        config.taint_source_all = false;
        let request = ScanRequest {
            config: Some(config.clone()),
            config_path: Some(PathBuf::from("custom.yaml")),
            taint_source_all: Some(true),
            ..ScanRequest::default()
        };

        let loaded = load_config(&request).expect("provided config should load");

        assert!(loaded.config.taint_source_all);
        assert!(loaded.info.is_none());
        assert_eq!(loaded.path, Some(PathBuf::from("custom.yaml")));
    }

    #[test]
    fn scan_rejects_missing_paths_before_running() {
        let request = ScanRequest {
            paths: vec![PathBuf::from("/definitely/missing/phanalist/path")],
            config: Some(Config::default()),
            ..ScanRequest::default()
        };

        let error = scan(request, None, Some(|_| {})).expect_err("missing path should fail");

        assert!(matches!(error, ScanError::InvalidPath(_)));
    }
}
