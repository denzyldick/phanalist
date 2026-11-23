use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use colored::Colorize;
use indicatif::ProgressBar;
use jwalk::WalkDir;
use mago_allocator::prelude::LocalArena;
use mago_syntax::cst::Statement;

use crate::callchain::CallChainIndex;
use crate::config::{Config, TestGenerationConfig};
use crate::debug_stats::{FileTimings, RuleTimings};
use crate::file::File;
use crate::outputs::codeclimate::CodeClimate;
use crate::outputs::json::Json;
use crate::outputs::sarif::Sarif;
use crate::outputs::text::Text;
use crate::outputs::Format;
use crate::outputs::OutputFormatter;
use crate::resource::{ResourceClass, ResourceFinding, ResourceService, ResourceSettings};
use crate::results::{
    EntrypointInfo, FindingContext, FlowEdge, FlowEdgeKind, FlowNode, FlowNodeKind, Results,
    ScanIntelligence, Violation,
};
use crate::rules::meta::{Confidence, FindingMeta};
use crate::rules::Rule;
use crate::rules::{self};
use crate::security::{CallGraph, FileTaint, SecurityService, SourceFilter};
use crate::verification::{testgen, Finding, FindingClass, Settings, VerificationService};

struct FileOutcome {
    path: PathBuf,
    violations: Vec<Violation>,
    timings: Option<FileTimings>,
}

/// Print a verbose line. When a progress bar is active, route it through
/// `ProgressBar::println` so the bar stays pinned to the bottom and the line
/// scrolls above it; otherwise fall back to plain stderr.
fn log_line(bar: Option<&ProgressBar>, msg: String) {
    match bar {
        // Only the drawn bar can pin itself to the bottom. When stderr isn't a
        // TTY the bar is hidden and `println` is a no-op, so fall back to
        // `eprintln!` to keep verbose output visible when piped to a file.
        Some(pb) if !pb.is_hidden() => pb.println(msg),
        _ => eprintln!("{msg}"),
    }
}

pub(crate) fn scan_worker_count(file_count: usize) -> usize {
    if file_count <= 1 {
        return file_count;
    }

    let configured = std::env::var("PHANALIST_JOBS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|workers| *workers > 0);

    configured
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|count| count.get())
                .unwrap_or(4)
        })
        .min(file_count)
        .max(1)
}

/// Walk `current_dir` and return the path of every non-excluded PHP file.
///
/// Only paths are collected, never file contents or ASTs: the caller re-reads
/// each file per pass and drops its AST before moving on, so a full-project
/// scan (`--src .`) holds one file at a time rather than the whole tree. The
/// returned list costs a few tens of bytes per file, which is negligible next
/// to the per-file ASTs it replaces.
///
/// Entries that cannot be walked (unreadable directories, broken symlinks,
/// over-long paths) are skipped rather than aborting the scan: a scan rooted at
/// `.` routinely walks into trees the process has no business reading.
pub fn collect_php_files(
    current_dir: PathBuf,
    verbose: u8,
    bar: Option<&ProgressBar>,
    exclude_paths: &[String],
) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for entry in WalkDir::new(current_dir).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                // Reported at `-v` rather than always: a scan rooted at `.`
                // walks into directories the process cannot read, and an
                // unconditional line per entry would drown the real output.
                if verbose >= 1 {
                    let where_ = err
                        .path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "?".to_string());
                    log_line(bar, format!("[skip] cannot read {where_}: {err}"));
                }
                continue;
            }
        };

        let path = entry.path();
        if !is_scannable_file(entry.file_type(), &path) {
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "php") {
            continue;
        }
        if !exclude_paths.is_empty()
            && crate::paths::is_excluded(&crate::paths::normalize_relative(&path), exclude_paths)
        {
            if verbose >= 2 {
                log_line(bar, format!("[vv] excluded {}", path.display()));
            }
            continue;
        }

        if verbose >= 2 {
            log_line(bar, format!("[vv] found {}", path.display()));
        }
        paths.push(path);
    }

    paths
}

/// True when a walked entry should be treated as a regular file.
///
/// `WalkDir::follow_links(false)` stops us descending into symlinked
/// directories but must not stop us reading a symlinked PHP file, so symlinks
/// still need a `stat()` to resolve. Every other entry is classified from the
/// directory listing the walk already performed, saving a syscall per file.
fn is_scannable_file(file_type: fs::FileType, path: &Path) -> bool {
    if file_type.is_file() {
        return true;
    }
    if file_type.is_symlink() {
        return fs::metadata(path).is_ok_and(|m| m.is_file());
    }
    false
}

/// Read a source file, returning `None` when it cannot be read (for example a
/// dangling path or a file that disappeared between the walk and the read).
pub(crate) fn read_source(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

pub struct Analyse {
    pub(crate) rules: HashMap<String, Box<dyn Rule>>,
    verification: VerificationService,
    resource: ResourceService,
    security: SecurityService,
    callchain: CallChainIndex,
    source_filter: SourceFilter,
    test_generation: Option<TestGenerationConfig>,
}

impl Analyse {
    pub fn new(config: &Config) -> Self {
        let source_filter = if config.taint_source_all {
            SourceFilter::All
        } else {
            SourceFilter::Request
        };
        let mut security = SecurityService::new();
        security = security.with_source_filter(source_filter);
        Self {
            rules: Self::get_active_rules(config),
            verification: VerificationService::new(),
            resource: ResourceService::new(),
            security,
            callchain: CallChainIndex::default(),
            source_filter,
            test_generation: config.test_generation.clone(),
        }
    }

    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn scan(
        &self,
        path: String,
        config: &Config,
        show_bar: bool,
        format: &Format,
        verbose: u8,
        collect_rule_metrics: bool,
        external_bar: Option<ProgressBar>,
    ) -> Results {
        self.prepare_project(
            std::slice::from_ref(&path),
            config,
            verbose,
            external_bar.as_ref(),
        );
        self.scan_prepared(
            path,
            config,
            show_bar,
            format,
            verbose,
            collect_rule_metrics,
            external_bar,
        )
    }

    /// Index every file reachable from `paths` before validation starts.
    ///
    /// Cross-file rules need one project-wide indexing pass. Keeping this
    /// separate from `scan_prepared` lets the CLI index all `--src` roots once
    /// instead of re-indexing each root independently.
    pub(crate) fn prepare_project(
        &self,
        paths: &[String],
        config: &Config,
        verbose: u8,
        bar: Option<&ProgressBar>,
    ) {
        let mut seen = HashSet::new();
        let mut files = Vec::new();
        for root in paths {
            for file in collect_php_files(PathBuf::from(root), verbose, bar, &config.exclude_paths)
            {
                let key = crate::paths::normalize_relative(&file);
                if seen.insert(key) {
                    files.push(file);
                }
            }
        }

        self.begin_index();

        let worker_count = scan_worker_count(files.len());
        if worker_count <= 1 {
            let mut arena = LocalArena::new();
            for file_path in &files {
                if verbose >= 3 {
                    log_line(bar, format!("[vvv] indexing {}", file_path.display()));
                }
                let Some(content) = read_source(file_path) else {
                    continue;
                };

                {
                    let file = File::new(&arena, file_path.clone(), content);
                    self.index_file(&file);
                }

                arena.reset();
            }
        } else {
            if self.has_verification_rules()
                || self.has_resource_rules()
                || self.has_security_rules()
            {
                let next = AtomicUsize::new(0);
                std::thread::scope(|scope| {
                    for _ in 0..worker_count {
                        let next = &next;
                        let files = &files;
                        scope.spawn(move || {
                            let mut arena = LocalArena::new();
                            loop {
                                let index = next.fetch_add(1, Ordering::Relaxed);
                                let Some(file_path) = files.get(index) else {
                                    break;
                                };
                                if verbose >= 3 {
                                    log_line(
                                        bar,
                                        format!("[vvv] indexing {}", file_path.display()),
                                    );
                                }
                                let Some(content) = read_source(file_path) else {
                                    continue;
                                };

                                {
                                    let file = File::new(&arena, file_path.clone(), content);
                                    self.index_service_file(&file);
                                }

                                arena.reset();
                            }
                        });
                    }
                });
            }

            let mut arena = LocalArena::new();
            for file_path in &files {
                let Some(content) = read_source(file_path) else {
                    continue;
                };

                {
                    let file = File::new(&arena, file_path.clone(), content);
                    self.index_rule_file(&file);
                }

                arena.reset();
            }
        }

        self.finalize_index();
    }

    pub(crate) fn begin_index(&self) {
        for rule in self.rules.values() {
            if rule.verification_class().is_none() {
                rule.begin_index();
            }
        }
        if self.has_verification_rules() {
            self.verification.begin_index();
        }
        if self.has_resource_rules() {
            self.resource.begin_index();
        }
        if self.has_security_rules() {
            self.security.begin_index();
        }
        self.callchain.reset();
    }

    pub(crate) fn index_rule_file(&self, file: &File<'_>) {
        for rule in self.rules.values() {
            if rule.verification_class().is_none() {
                rule.index_file(file);
            }
        }
    }

    pub(crate) fn index_service_file(&self, file: &File<'_>) {
        self.callchain.index_file(file);
        if self.has_verification_rules() {
            self.verification.index_file(file);
        }
        if self.has_resource_rules() {
            self.resource.index_file(file);
        }
        if self.has_security_rules() {
            self.security.index_file(file);
        }
    }

    pub(crate) fn index_file(&self, file: &File<'_>) {
        self.index_rule_file(file);
        self.index_service_file(file);
    }

    pub(crate) fn finalize_index(&self) {
        self.finalize_index_with_progress(|_| {});
    }

    pub(crate) fn finalize_index_with_progress<F: FnMut(&str)>(&self, mut on_step: F) {
        let rule_count = self
            .rules
            .values()
            .filter(|rule| rule.verification_class().is_none())
            .count();
        let mut idx = 0usize;
        for rule in self.rules.values() {
            if rule.verification_class().is_none() {
                idx += 1;
                on_step(&format!(
                    "Finalizing rule index {}/{} ({})...",
                    idx,
                    rule_count,
                    rule.get_code()
                ));
                rule.finalize_index();
            }
        }
        if self.has_verification_rules() {
            on_step("Finalizing verification index...");
            self.verification.finalize_index();
        }
        if self.has_resource_rules() {
            on_step("Finalizing resource index...");
            self.resource.finalize_index();
        }
        if self.has_security_rules() {
            on_step("Finalizing security/taint index (cross-file taint graph)... this can take a long time on large projects");
            self.security.finalize_index();
        }
        on_step("Index finalization complete");
    }

    /// Drop cross-file rule state for a file removed by the editor.
    pub(crate) fn remove_indexed_file(&self, path: &Path) {
        for rule in self.rules.values() {
            if rule.verification_class().is_none() {
                rule.remove_file(path);
            }
        }
        if self.has_verification_rules() {
            self.verification.remove_file(path);
        }
        if self.has_resource_rules() {
            self.resource.remove_file(path);
        }
        if self.has_security_rules() {
            self.security.remove_file(path);
        }
    }

    pub(crate) fn has_security_rules(&self) -> bool {
        self.rules.values().any(|rule| rule.taint_mode().is_some())
    }

    pub(crate) fn has_verification_rules(&self) -> bool {
        self.rules
            .values()
            .any(|rule| rule.verification_class().is_some())
    }

    pub(crate) fn has_resource_rules(&self) -> bool {
        self.rules
            .values()
            .any(|rule| rule.resource_class().is_some())
    }

    /// Rebuild cross-file rule state for a single file from its on-disk content.
    ///
    /// Editors close documents that still exist, so their summaries must be
    /// restored instead of discarded.
    pub(crate) fn index_file_from_disk(&self, path: &Path) {
        let Some(content) = read_source(path) else {
            self.remove_indexed_file(path);
            return;
        };

        let arena = LocalArena::new();
        let file = File::new(&arena, path.to_path_buf(), content);
        self.index_file(&file);
        self.finalize_index();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scan_prepared(
        &self,
        path: String,
        config: &Config,
        show_bar: bool,
        format: &Format,
        verbose: u8,
        collect_rule_metrics: bool,
        external_bar: Option<ProgressBar>,
    ) -> Results {
        let now = std::time::Instant::now();
        let mut results = Results::default();
        if collect_rule_metrics {
            results.rule_timings = Some(RuleTimings::default());
        }

        let has_external_bar = external_bar.is_some();

        if show_bar && format == &Format::text && !has_external_bar {
            println!();
            println!("Scanning files in {} ...", path.to_string().bold());
        }

        if verbose >= 1 {
            for pattern in crate::paths::missing_literal_excludes(&config.exclude_paths) {
                log_line(
                    external_bar.as_ref(),
                    format!("exclude_paths: '{pattern}' matches no existing path — typo?")
                        .yellow()
                        .to_string(),
                );
            }
        }

        // 1. Resolve the work list. Paths only: contents and ASTs are read one
        //    file at a time below, so a scan rooted at `.` stays bounded no
        //    matter how many files the project tree holds.
        let paths = collect_php_files(
            PathBuf::from(&path),
            verbose,
            external_bar.as_ref(),
            &config.exclude_paths,
        );

        // The bar now counts exactly the files that will be analysed, instead of
        // every directory entry the walk passed through.
        let progress_bar = if let Some(pb) = external_bar {
            Some(pb)
        } else if show_bar && format == &Format::text {
            Some(Self::progress_bar(paths.len() as u64))
        } else {
            None
        };

        let bar_active = progress_bar.is_some();

        let worker_count = scan_worker_count(paths.len());
        let mut files = 0;

        if worker_count <= 1 {
            // One arena is reused for the whole scan, but reclaimed after every
            // file. `reset` frees each file's AST while keeping the chunks for
            // reuse, so the arena settles at the size of the largest single file
            // instead of growing to the size of the entire project.
            let mut arena = LocalArena::new();

            for file_path in &paths {
                if verbose >= 2 {
                    log_line(
                        progress_bar.as_ref(),
                        format!("[vv] parsing {}", file_path.display()),
                    );
                }
                let Some(content) = read_source(file_path) else {
                    continue;
                };

                if verbose >= 1 {
                    log_line(
                        progress_bar.as_ref(),
                        format!("[v] analysing {}", file_path.display()),
                    );
                }
                if let Some(ref pb) = progress_bar {
                    pb.inc(1);
                }

                let (analysed_path, file_timings) = {
                    let mut file = File::new(&arena, file_path.clone(), content);
                    let (violations, file_timings) =
                        self.analyse_file(&mut file, collect_rule_metrics);
                    let analysed_path = file.path.display().to_string();
                    results.add_file_violations(&file, violations);
                    (analysed_path, file_timings)
                };

                arena.reset();

                if let (Some(rt), Some(ft)) = (results.rule_timings.as_mut(), file_timings) {
                    rt.merge_file(analysed_path, ft);
                }

                files += 1;
            }
        } else {
            // 2. Main pass in parallel. Each worker keeps one file's AST live at
            //    a time; the arena is reset between files. The work queue is
            //    atomic so large files do not strand a fixed chunk.
            let next = AtomicUsize::new(0);
            let (tx, rx) = mpsc::channel::<FileOutcome>();

            std::thread::scope(|scope| {
                for _ in 0..worker_count {
                    let tx = tx.clone();
                    let bar = progress_bar.clone();
                    let next = &next;
                    let paths = &paths;

                    scope.spawn(move || {
                        let mut arena = LocalArena::new();
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(file_path) = paths.get(index) else {
                                break;
                            };

                            if verbose >= 2 {
                                log_line(
                                    bar.as_ref(),
                                    format!("[vv] parsing {}", file_path.display()),
                                );
                            }
                            let Some(content) = read_source(file_path) else {
                                continue;
                            };

                            if verbose >= 1 {
                                log_line(
                                    bar.as_ref(),
                                    format!("[v] analysing {}", file_path.display()),
                                );
                            }
                            if let Some(ref pb) = bar {
                                pb.inc(1);
                            }

                            let outcome = {
                                let mut file = File::new(&arena, file_path.clone(), content);
                                let (violations, timings) =
                                    self.analyse_file(&mut file, collect_rule_metrics);
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

                for outcome in rx {
                    let FileOutcome {
                        path,
                        violations,
                        timings,
                    } = outcome;
                    let path_string = timings.as_ref().map(|_| path.display().to_string());
                    results.add_file_violations_for_path(path, violations);

                    if let (Some(rt), Some(ft), Some(path_string)) =
                        (results.rule_timings.as_mut(), timings, path_string)
                    {
                        rt.merge_file(path_string, ft);
                    }

                    files += 1;
                }
            });
        }

        if bar_active && !has_external_bar {
            progress_bar.unwrap().finish();
        }

        self.apply_taint_tiering(&mut results);

        results.total_files_count = files;
        results.duration = Some(now.elapsed());

        results
    }

    pub(crate) fn parse_config(config_path: String, output_format: &Format, quiet: bool) -> Config {
        let path = PathBuf::from(config_path);
        let default_config = Config::default();

        let output_hints = !quiet && output_format != &Format::json;
        match fs::read_to_string(&path) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                if let Err(e) = default_config.save(&path) {
                    if output_format == &Format::text {
                        println!(
                            "Unable to save {} configuration file, error: {}",
                            path.display().to_string().bold(),
                            e
                        );
                    }
                } else if output_hints && output_format == &Format::text {
                    println!(
                        "The new {} configuration file as been created",
                        path.display().to_string().bold()
                    );
                }

                default_config
            }

            Err(e) => {
                panic!("{}", e)
            }

            Ok(s) => {
                if output_hints && output_format == &Format::text {
                    println!(
                        "Using configuration file {}",
                        path.display().to_string().bold()
                    );
                }

                match serde_yaml::from_str::<Config>(&s) {
                    Ok(mut c) => {
                        let default = Config::default();
                        for (code, settings) in default.rules {
                            c.rules.entry(code).or_insert(settings);
                        }
                        c
                    }
                    Err(e) => {
                        if output_format == &Format::text {
                            println!("Unable to use the config: {}. Ignoring it.", e);
                        }
                        default_config
                    }
                }
            }
        }
    }

    // Called from main.rs; dead_code is a false positive across crate targets.
    #[allow(dead_code)]
    pub(crate) fn output(&mut self, results: &mut Results, format: Format, summary_only: bool) {
        if summary_only {
            results.files = HashMap::new();
        };

        // Baseline filtering can leave a file with nothing to report. Prune in
        // place: cloning the map first would duplicate every violation, which
        // is the largest allocation of the whole run.
        results.files.retain(|_, violations| !violations.is_empty());

        let effective_settings = self.effective_rule_settings();
        match format {
            Format::json => Json::output(results),
            Format::sarif => Sarif::output_with_settings(results, &effective_settings),
            Format::codeclimate => CodeClimate::output_with_settings(results, &effective_settings),
            _ => Text::output(results),
        };
    }

    pub(crate) fn effective_rule_settings(&self) -> HashMap<String, serde_json::Value> {
        let all_rules = rules::all_rules();
        let mut effective = HashMap::new();
        for (code, rule) in all_rules {
            let settings = self
                .rules
                .get(&code)
                .and_then(|configured| configured.settings_json())
                .or_else(|| rule.settings_json());
            if let Some(settings) = settings {
                effective.insert(code, settings);
            }
        }
        effective
    }

    /// Apply interprocedural taint tiering and request-only filtering to results.
    ///
    /// This mirrors the post-processing performed at the end of `scan_prepared`
    /// and is exposed for alternative/progress-aware scan drivers.
    pub(crate) fn apply_taint_tiering(&self, results: &mut Results) {
        if !self.has_security_rules() {
            return;
        }

        let request_rooted = self.security.compute_request_rooted_functions();
        let request_only = self.source_filter == SourceFilter::Request;
        results.files.retain(|_, violations| {
            violations.retain_mut(|v| {
                match &mut v.taint_source {
                    Some(ts) => {
                        // Keep if request_rooted is true (top-level code or
                        // request-derived function call), OR if the enclosing
                        // function is in the request-rooted set.
                        let reachable = ts.request_rooted
                            || ts
                                .enclosing_function
                                .as_ref()
                                .map(|f| request_rooted.contains(&f.to_ascii_lowercase()))
                                .unwrap_or(false);
                        if reachable && !ts.request_rooted {
                            // Cross-file propagation: the sink itself only
                            // saw a parameter, but a caller supplied request
                            // data. Promote the finding to the proven tier.
                            ts.request_rooted = true;
                            ts.source_kind = Some(String::from("request"));
                            let fingerprint = v.meta.fingerprint.clone();
                            v.meta =
                                FindingMeta::from_rule(&v.rule, fingerprint).with_proven_taint();
                        }
                        !request_only || reachable
                    }
                    None => true, // Non-taint rules (E0000-E0052) always pass
                }
            });
            !violations.is_empty()
        });

        // Request-only mode dropped leads after their per-rule counts were
        // accumulated; refresh the summary counts to match what is left.
        if request_only {
            results.recount_codes();
        }
    }

    /// Build the cross-cutting intelligence model used by Pro's Findings and
    /// Flow Map views. This is bounded so very large projects stay renderable.
    pub fn build_intelligence(&self, results: &Results) -> ScanIntelligence {
        const MAX_FUNCTIONS: usize = 220;
        const MAX_EDGES: usize = 700;

        let graph = self.security.call_graph();
        let source_functions = self.security.find_source_functions();
        let sources: Vec<String> = source_functions
            .iter()
            .map(|s| normalize_function(s))
            .collect();
        let request_rooted: HashSet<String> = self
            .security
            .compute_request_rooted_functions()
            .into_iter()
            .map(|s| normalize_function(&s))
            .collect();
        let function_files = self.security.function_files();
        let display_names: HashMap<String, String> = self
            .security
            .all_function_names()
            .into_iter()
            .map(|name| (normalize_function(&name), name))
            .collect();

        let entrypoint_candidates = derive_entrypoint_candidates(&graph, &display_names);

        let mut entrypoints = Vec::new();
        let mut entrypoint_functions = HashSet::new();
        let mut seen_entrypoints = HashSet::new();
        for key in entrypoint_candidates.into_iter().take(80) {
            if !entrypoint_functions.insert(key.clone()) {
                continue;
            }
            let file = function_files.get(&key).cloned();
            let label = display_names
                .get(&key)
                .cloned()
                .unwrap_or_else(|| key.clone());
            let (kind, confidence) = classify_entrypoint(&label, file.as_deref());
            let id = format!("entry:{key}");
            if seen_entrypoints.insert(id.clone()) {
                entrypoints.push(EntrypointInfo {
                    id,
                    label,
                    kind,
                    file,
                    line: None,
                    confidence,
                    finding_count: 0,
                });
            }
        }

        let mut target_functions = BTreeSet::new();
        for violations in results.files.values() {
            for violation in violations {
                let function = violation
                    .taint_source
                    .as_ref()
                    .and_then(|source| source.enclosing_function.clone())
                    .or_else(|| violation.call_chain.last().cloned());
                if let Some(function) = function {
                    target_functions.insert(normalize_function(&function));
                }
            }
        }

        let mut relevant: BTreeSet<String> = BTreeSet::new();
        for function in entrypoint_functions.iter() {
            relevant.insert(function.clone());
        }
        for function in &target_functions {
            relevant.insert(function.clone());
        }

        let mut frontier: Vec<String> = target_functions.iter().cloned().collect();
        for _ in 0..3 {
            let mut next = Vec::new();
            for function in frontier {
                if let Some(callers) = graph.callers(&function) {
                    for caller in callers {
                        let caller = normalize_function(caller);
                        if relevant.insert(caller.clone()) {
                            next.push(caller);
                        }
                    }
                }
            }
            frontier = next;
        }

        let mut relevant_vec: Vec<String> = relevant.into_iter().collect();
        if relevant_vec.len() > MAX_FUNCTIONS {
            let mut kept: Vec<String> = entrypoint_functions.iter().cloned().collect();
            for function in relevant_vec {
                if kept.len() >= MAX_FUNCTIONS {
                    break;
                }
                if !kept.contains(&function) {
                    kept.push(function);
                }
            }
            relevant_vec = kept;
        }
        let relevant_set: HashSet<String> = relevant_vec.iter().cloned().collect();
        let entrypoint_paths_by_function = entrypoint_paths(&graph, &entrypoints, &relevant_set);

        let mut finding_context = HashMap::new();
        let mut function_risk: HashMap<String, u8> = HashMap::new();

        for (file, violations) in &results.files {
            for violation in violations {
                let fingerprint = if violation.meta.fingerprint.is_empty() {
                    format!(
                        "{}|{}|{}|{}",
                        violation.rule, file, violation.start_line, violation.start_column
                    )
                } else {
                    violation.meta.fingerprint.clone()
                };

                let function = violation
                    .taint_source
                    .as_ref()
                    .and_then(|source| source.enclosing_function.clone())
                    .or_else(|| violation.call_chain.last().cloned())
                    .map(|name| normalize_function(&name));
                let proven = violation
                    .taint_source
                    .as_ref()
                    .is_some_and(|source| source.request_rooted);
                let reachable = function
                    .as_ref()
                    .map(|name| sources.contains(name) || request_rooted.contains(name))
                    .unwrap_or(false)
                    || proven;
                let path = if reachable {
                    function
                        .as_ref()
                        .map(|name| reachability_path(&graph, &sources, name))
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };

                let mut entrypoint_ids = Vec::new();
                if let Some(function) = &function {
                    if let Some(ids) = entrypoint_paths_by_function.get(function) {
                        entrypoint_ids.extend(ids.iter().take(4).cloned());
                    }
                }

                let (risk_score, risk_reasons) = score_finding(violation, reachable, proven);
                if let Some(function) = &function {
                    function_risk
                        .entry(function.clone())
                        .and_modify(|score| *score = (*score).max(risk_score))
                        .or_insert(risk_score);
                }

                let mut impact = Vec::new();
                if let Some(cwe) = &violation.meta.cwe {
                    impact.push(cwe.clone());
                }
                if let Some(owasp) = &violation.meta.owasp {
                    impact.push(owasp.clone());
                }
                if violation.meta.category.as_str() == "security" {
                    impact.push("security-control-failure".to_string());
                }

                let mut missing_controls = Vec::new();
                if violation.taint_source.is_some() {
                    missing_controls
                        .push("sanitization not proven in recorded evidence".to_string());
                }

                let mut data_classes = Vec::new();
                if let Some(source) = &violation.taint_source {
                    if source.request_rooted {
                        data_classes.push("request-input".to_string());
                    } else {
                        data_classes.push("function-parameter".to_string());
                    }
                }

                finding_context.insert(
                    fingerprint.clone(),
                    FindingContext {
                        fingerprint,
                        rule: violation.rule.clone(),
                        file: file.clone(),
                        line: violation.start_line,
                        reachable,
                        reachability_path: path,
                        entrypoint_ids,
                        risk_score,
                        risk_reasons,
                        impact,
                        missing_controls,
                        data_classes,
                    },
                );
            }
        }

        for entry in &mut entrypoints {
            entry.finding_count = finding_context
                .values()
                .filter(|context| context.entrypoint_ids.contains(&entry.id))
                .count();
        }

        let mut nodes = Vec::new();
        for function in &relevant_vec {
            let is_entrypoint = entrypoint_functions.contains(function);
            let id = if is_entrypoint {
                format!("entry:{function}")
            } else {
                format!("fn:{function}")
            };
            nodes.push(FlowNode {
                label: display_names
                    .get(function)
                    .cloned()
                    .unwrap_or_else(|| function.clone()),
                id,
                kind: if is_entrypoint {
                    FlowNodeKind::Entrypoint
                } else {
                    FlowNodeKind::Function
                },
                file: function_files.get(function).cloned(),
                line: None,
                function: Some(function.clone()),
                risk_score: function_risk.get(function).copied().unwrap_or(0),
            });
        }

        for function in &source_functions {
            let key = normalize_function(function);
            if relevant_set.contains(&key) {
                continue;
            }
            nodes.push(FlowNode {
                id: format!("source:{key}"),
                label: format!("Request source: {function}"),
                kind: FlowNodeKind::Source,
                file: function_files.get(&key).cloned(),
                line: None,
                function: Some(key),
                risk_score: 70,
            });
        }

        for context in finding_context.values() {
            nodes.push(FlowNode {
                id: format!("sink:{}", context.fingerprint),
                label: format!("{} · {}:{}", context.rule, context.file, context.line),
                kind: FlowNodeKind::Sink,
                file: Some(context.file.clone()),
                line: Some(context.line),
                function: None,
                risk_score: context.risk_score,
            });
        }

        let node_id = |function: &str| {
            if entrypoint_functions.contains(function) {
                format!("entry:{function}")
            } else {
                format!("fn:{function}")
            }
        };

        let node_ids: HashSet<String> = nodes.iter().map(|node| node.id.clone()).collect();
        let mut edges = Vec::new();

        // Reserve most of the budget for finding evidence. Call edges are
        // useful context, but must never crowd out the actual source→sink path.
        let mut contexts: Vec<&FindingContext> = finding_context.values().collect();
        contexts.sort_by(|a, b| {
            b.risk_score
                .cmp(&a.risk_score)
                .then_with(|| a.fingerprint.cmp(&b.fingerprint))
        });
        for context in contexts {
            if let Some(function) = context.reachability_path.last() {
                let from = node_id(&normalize_function(function));
                let to = format!("sink:{}", context.fingerprint);
                if node_ids.contains(&from) && node_ids.contains(&to) {
                    edges.push(FlowEdge {
                        from,
                        to: to.clone(),
                        kind: FlowEdgeKind::Taint,
                        label: if context.reachable {
                            "proven flow".to_string()
                        } else {
                            "lead".to_string()
                        },
                        proven: context.reachable,
                        finding_fingerprint: Some(context.fingerprint.clone()),
                    });

                    // Connect the source node when the path starts at a source
                    // that was not kept in the function subgraph.
                    if let Some(source) = context.reachability_path.first() {
                        let source_key = normalize_function(source);
                        let source_id = if relevant_set.contains(&source_key) {
                            node_id(&source_key)
                        } else {
                            format!("source:{source_key}")
                        };
                        let next_id = context
                            .reachability_path
                            .get(1)
                            .map(|next| node_id(&normalize_function(next)))
                            .unwrap_or(to);
                        if node_ids.contains(&source_id) && node_ids.contains(&next_id) {
                            edges.push(FlowEdge {
                                from: source_id,
                                to: next_id,
                                kind: FlowEdgeKind::Reachability,
                                label: "request data".to_string(),
                                proven: context.reachable,
                                finding_fingerprint: Some(context.fingerprint.clone()),
                            });
                        }
                    }
                }
            }
            if edges.len() >= MAX_EDGES.saturating_sub(280) {
                break;
            }
        }

        // Deterministic call edges; HashMap iteration order must not decide
        // which flows survive truncation.
        let mut graph_edges: Vec<_> = graph.edges.iter().collect();
        graph_edges.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(b.1)));
        for (caller, callees) in graph_edges {
            for callee in callees {
                let caller = normalize_function(caller);
                let callee = normalize_function(callee);
                if !relevant_set.contains(&caller) || !relevant_set.contains(&callee) {
                    continue;
                }
                let from = node_id(&caller);
                let to = node_id(&callee);
                if !node_ids.contains(&from) || !node_ids.contains(&to) {
                    continue;
                }
                edges.push(FlowEdge {
                    from,
                    to,
                    kind: FlowEdgeKind::Call,
                    label: "calls".to_string(),
                    proven: false,
                    finding_fingerprint: None,
                });
                if edges.len() >= MAX_EDGES {
                    break;
                }
            }
            if edges.len() >= MAX_EDGES {
                break;
            }
        }

        ScanIntelligence {
            entrypoints,
            nodes,
            edges,
            finding_context,
        }
    }

    pub(crate) fn analyse_file(
        &self,
        file: &mut File<'_>,
        collect_rule_metrics: bool,
    ) -> (Vec<Violation>, Option<FileTimings>) {
        let mut violations: Vec<Violation> = vec![];
        let mut timings = if collect_rule_metrics {
            Some(FileTimings::new())
        } else {
            None
        };

        if let Some(program) = file.ast {
            file.reference_counter.build_reference_counter(program);

            let file_taint: Option<FileTaint> = if self.has_security_rules() {
                let (taint, cg) = self.security.analyze(file);
                self.security.merge_call_graph(cg);
                Some(taint)
            } else {
                None
            };

            let mut verification_groups: Vec<(Settings, Vec<FindingClass>)> = Vec::new();
            let mut resource_groups: Vec<(ResourceSettings, Vec<ResourceClass>)> = Vec::new();
            for rule in self.rules.values() {
                if let (Some(class), Some(settings)) =
                    (rule.verification_class(), rule.verification_settings())
                {
                    if let Some((_, classes)) = verification_groups
                        .iter_mut()
                        .find(|(existing, _)| existing == &settings)
                    {
                        classes.push(class);
                    } else {
                        verification_groups.push((settings, vec![class]));
                    }
                    continue;
                }
                if let (Some(class), Some(settings)) =
                    (rule.resource_class(), rule.resource_settings())
                {
                    if let Some((_, classes)) = resource_groups
                        .iter_mut()
                        .find(|(existing, _)| existing == &settings)
                    {
                        classes.push(class);
                    } else {
                        resource_groups.push((settings, vec![class]));
                    }
                    continue;
                }

                let rule_start = timings.as_ref().map(|_| Instant::now());
                let validated = match &file_taint {
                    Some(taint) => rule.validate_file_with_taint(file, taint),
                    None => rule.validate_file(file),
                };
                let Some(mut file_violations) = validated else {
                    continue;
                };
                violations.append(&mut file_violations);

                if let Some(t) = timings.as_mut() {
                    let elapsed = rule_start.unwrap().elapsed();
                    let entry = t.entry(rule.get_code()).or_default();
                    entry.duration += elapsed;
                    entry.validated = true;
                    entry.statements += 1;
                }
            }

            for (settings, classes) in verification_groups {
                let rule_start = timings.as_ref().map(|_| Instant::now());
                let findings = self.verification.analyze(file, &settings, &classes);
                violations.extend(findings.into_iter().map(|finding| {
                    self.violation_from_finding(file, finding, self.test_generation.as_ref())
                }));

                if let Some(t) = timings.as_mut() {
                    let elapsed = rule_start.unwrap().elapsed();
                    for class in classes {
                        let entry = t.entry(String::from(class.code())).or_default();
                        entry.duration += elapsed;
                        entry.validated = true;
                        entry.statements += program.statements.len();
                    }
                }
            }

            for (settings, classes) in resource_groups {
                let rule_start = timings.as_ref().map(|_| Instant::now());
                let findings = self.resource.analyze(file, &settings, &classes);
                violations.extend(
                    findings
                        .into_iter()
                        .map(|finding| Self::violation_from_resource_finding(file, finding)),
                );

                if let Some(t) = timings.as_mut() {
                    let elapsed = rule_start.unwrap().elapsed();
                    for class in classes {
                        let entry = t.entry(String::from(class.code())).or_default();
                        entry.duration += elapsed;
                        entry.validated = true;
                        entry.statements += program.statements.len();
                    }
                }
            }

            for statement in program.statements.iter() {
                violations.append(&mut self.analyse_file_statement(
                    file,
                    statement,
                    timings.as_mut(),
                ));
            }
        }
        (violations, timings)
    }

    fn violation_from_finding(
        &self,
        file: &File<'_>,
        mut finding: Finding,
        test_generation: Option<&TestGenerationConfig>,
    ) -> Violation {
        let mut call_chain = Vec::new();
        if let Some(witness) = finding.witness.as_mut() {
            if witness.call_chain.is_empty() {
                if let Some(callable) = &witness.callable {
                    let source = witness.source.to_string_lossy();
                    let target = format!("{}() at {}:{}", callable.name, source, witness.line);
                    witness.call_chain = self.callchain.chain_for(&callable.name, &target);
                }
            }
            call_chain = witness.call_chain.clone();
        }
        if call_chain.is_empty() {
            call_chain = self
                .callchain
                .chain_for_location(&file.path, file.line_number(finding.span.start.offset));
        }
        let generated_test = test_generation.and_then(|config| {
            finding.witness.as_ref().and_then(|witness| {
                testgen::generate_test(witness, Some(Path::new(&config.output_dir)))
            })
        });
        let mut violation =
            Self::violation_from_parts(file, finding.class.code(), finding.span, finding.message);
        violation.call_chain = call_chain;
        violation.generated_test = generated_test;
        violation
    }

    fn violation_from_resource_finding(file: &File<'_>, finding: ResourceFinding) -> Violation {
        Self::violation_from_parts(file, finding.class.code(), finding.span, finding.message)
    }

    fn violation_from_parts(
        file: &File<'_>,
        rule: &str,
        span: mago_span::Span,
        message: crate::results::Message,
    ) -> Violation {
        let start_line = file.line_number(span.start.offset);
        let start_column = file.column_number(span.start.offset);
        let end_line = file.line_number(span.end.offset);
        let end_column = file.column_number(span.end.offset);
        let line = file
            .lines
            .get(start_line.saturating_sub(1))
            .cloned()
            .unwrap_or_default();

        Violation {
            rule: String::from(rule),
            line,
            message,
            start_line,
            start_column,
            end_line,
            end_column,
            taint_source: None,
            call_chain: Vec::new(),
            meta: Default::default(),
            generated_test: None,
        }
    }

    fn get_active_rules(config: &Config) -> HashMap<String, Box<dyn Rule>> {
        let active_codes = Self::filter_active_codes(
            rules::all_rules().into_keys().collect(),
            &config.enabled_rules,
            &config.disable_rules,
        );

        let mut active_rules = rules::all_rules();
        active_rules.retain(|code, rule| {
            rule.read_config(config);

            active_codes.contains(code)
        });

        active_rules
    }

    fn filter_active_codes(
        all_codes: Vec<String>,
        enabled: &[String],
        disabled: &[String],
    ) -> Vec<String> {
        let mut filtered_codes = all_codes;

        if !enabled.is_empty() {
            filtered_codes.retain(|x| enabled.contains(x));
        }

        if !disabled.is_empty() {
            filtered_codes.retain(|x| !disabled.contains(x));
        }

        filtered_codes
    }

    fn progress_bar(total_files: u64) -> ProgressBar {
        ProgressBar::new(total_files)
    }

    pub fn analyse_file_statement<'a>(
        &self,
        file: &File<'a>,
        statement: &Statement<'a>,
        mut timings: Option<&mut FileTimings>,
    ) -> Vec<Violation> {
        let mut violations = Vec::new();

        for rule in self.rules.values() {
            let rule_start = timings.as_ref().map(|_| Instant::now());

            let validated = rule.do_validate(file);
            let mut stmt_count = 0;
            if validated {
                let flat = rule.flatten_statements_to_validate(statement);
                stmt_count = flat.len();
                for statement in flat {
                    violations.append(&mut rule.validate(file, statement));
                }
            }

            if let Some(t) = timings.as_deref_mut() {
                let elapsed = rule_start.unwrap().elapsed();
                let entry = t.entry(rule.get_code()).or_default();
                entry.duration += elapsed;
                entry.validated |= validated;
                entry.statements += stmt_count;
            }
        }

        violations
    }
}

fn normalize_function(name: &str) -> String {
    name.trim().trim_start_matches('\\').to_ascii_lowercase()
}

/// Functions that nothing calls locally are the strongest available signal for
/// externally invoked entrypoints (routes, commands, listeners).
fn derive_entrypoint_candidates(
    graph: &CallGraph,
    display_names: &HashMap<String, String>,
) -> BTreeSet<String> {
    let called: HashSet<String> = graph
        .edges
        .values()
        .flatten()
        .chain(graph.request_args.keys())
        .cloned()
        .collect();
    let mut candidates: BTreeSet<String> = graph
        .entry_points
        .iter()
        .map(|name| normalize_function(name))
        .collect();
    candidates.extend(
        display_names
            .keys()
            .filter(|name| !called.contains(*name))
            .cloned(),
    );
    candidates
}

fn contains_word(haystack: &str, word: &str) -> bool {
    haystack
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '\\')
        .any(|token| token.eq_ignore_ascii_case(word))
}

fn classify_entrypoint(function: &str, file: Option<&str>) -> (String, u8) {
    let haystack = format!("{} {}", file.unwrap_or_default(), function).to_ascii_lowercase();
    if contains_word(&haystack, "controller")
        || contains_word(&haystack, "action")
        || contains_word(&haystack, "route")
        || function.eq_ignore_ascii_case("index")
    {
        ("HTTP route".to_string(), 90)
    } else if haystack.contains("command") || haystack.contains("console") {
        ("CLI command".to_string(), 82)
    } else if haystack.contains("listener")
        || haystack.contains("subscriber")
        || haystack.contains("job")
        || haystack.contains("worker")
    {
        ("Job / listener".to_string(), 76)
    } else {
        ("Candidate entrypoint".to_string(), 48)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathDirection {
    /// Walk callers: a tainted callee propagates up into the function that calls it.
    Callers,
    /// Walk callees: an entrypoint reaches the functions it calls.
    Callees,
}

fn call_path(
    graph: &CallGraph,
    roots: &[String],
    target: &str,
    direction: PathDirection,
) -> Vec<String> {
    let target = normalize_function(target);

    for root in roots {
        let root = normalize_function(root);
        if root == target {
            return vec![root];
        }

        let mut visited = HashSet::new();
        let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
        queue.push_back((root.clone(), vec![root.clone()]));

        while let Some((current, path)) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue;
            }
            let neighbors = match direction {
                PathDirection::Callers => graph.callers(&current),
                PathDirection::Callees => graph.callees(&current),
            };
            let Some(neighbors) = neighbors else {
                continue;
            };
            for neighbor in neighbors {
                let neighbor = normalize_function(neighbor);
                if neighbor == target {
                    let mut found = path.clone();
                    found.push(neighbor);
                    return found;
                }
                if !visited.contains(&neighbor) {
                    let mut next = path.clone();
                    next.push(neighbor.clone());
                    queue.push_back((neighbor, next));
                }
            }
        }
    }
    Vec::new()
}

/// Path from a tainted source up to the function that consumes it.
fn reachability_path(graph: &CallGraph, roots: &[String], target: &str) -> Vec<String> {
    call_path(graph, roots, target, PathDirection::Callers)
}

/// Precompute which entrypoints can reach each relevant function.
///
/// This avoids a BFS per finding per entrypoint on large projects, and uses the
/// downstream (`callees`) direction because an entrypoint calls into the code.
fn entrypoint_paths(
    graph: &CallGraph,
    entrypoints: &[EntrypointInfo],
    relevant: &HashSet<String>,
) -> HashMap<String, Vec<String>> {
    let mut paths: HashMap<String, Vec<String>> = HashMap::new();

    for entry in entrypoints {
        let root = entry.id.trim_start_matches("entry:").to_string();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(root);

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current.clone()) {
                continue;
            }
            paths
                .entry(current.clone())
                .or_default()
                .push(entry.id.clone());

            let Some(callees) = graph.callees(&current) else {
                continue;
            };
            for callee in callees {
                let callee = normalize_function(callee);
                if relevant.contains(&callee) && !visited.contains(&callee) {
                    queue.push_back(callee);
                }
            }
        }
    }

    paths
}

fn score_finding(violation: &Violation, reachable: bool, proven: bool) -> (u8, Vec<String>) {
    let mut score = violation.meta.severity.weight() as u16 * 9;
    let mut reasons = vec![format!("{} severity", violation.meta.severity.as_str())];

    if proven {
        score += 20;
        reasons.push("proven request taint".to_string());
    } else if violation.taint_source.is_some() {
        score += 7;
        reasons.push("taint lead".to_string());
    }

    if reachable {
        score += 14;
        reasons.push("reachable from a request root".to_string());
    }

    if violation.meta.confidence == Confidence::High {
        score += 5;
        reasons.push("high confidence".to_string());
    }
    if let Some(cwe) = &violation.meta.cwe {
        score += 5;
        reasons.push(cwe.clone());
    }
    if let Some(owasp) = &violation.meta.owasp {
        score += 4;
        reasons.push(owasp.clone());
    }

    (score.min(100) as u8, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entrypoints_include_defined_but_uncalled_functions() {
        let mut graph = CallGraph::new();
        graph.add_edge("Controller::index", "Service::run");
        let display_names = HashMap::from([
            (
                "controller::index".to_string(),
                "Controller::index".to_string(),
            ),
            ("service::run".to_string(), "Service::run".to_string()),
            ("worker::nightly".to_string(), "Worker::nightly".to_string()),
        ]);

        let candidates = derive_entrypoint_candidates(&graph, &display_names);
        assert!(candidates.contains("controller::index"));
        assert!(candidates.contains("worker::nightly"));
        assert!(!candidates.contains("service::run"));
    }

    #[test]
    fn classify_entrypoint_ignores_action_inside_other_words() {
        let (kind, confidence) =
            classify_entrypoint("TransactionService", Some("src/TransactionService.php"));
        assert_eq!(kind, "Candidate entrypoint");
        assert_eq!(confidence, 48);
    }

    #[test]
    fn call_paths_follow_the_requested_direction() {
        let mut graph = CallGraph::new();
        graph.add_edge("Controller::index", "Service::run");
        graph.add_edge("Service::run", "Db::query");
        let roots = vec!["Controller::index".to_string()];

        assert_eq!(
            call_path(&graph, &roots, "DB::Query", PathDirection::Callees),
            vec![
                "controller::index".to_string(),
                "service::run".to_string(),
                "db::query".to_string()
            ]
        );
        assert!(call_path(&graph, &roots, "db::query", PathDirection::Callers).is_empty());
    }

    #[test]
    fn entrypoint_paths_precompute_downstream_reachability() {
        let mut graph = CallGraph::new();
        graph.add_edge("Controller::index", "Service::run");
        graph.add_edge("Service::run", "Db::query");

        let entrypoints = vec![EntrypointInfo {
            id: "entry:controller::index".to_string(),
            label: "Controller::index".to_string(),
            kind: "HTTP route".to_string(),
            file: None,
            line: None,
            confidence: 90,
            finding_count: 0,
        }];
        let relevant = HashSet::from([
            "controller::index".to_string(),
            "service::run".to_string(),
            "db::query".to_string(),
        ]);

        let paths = entrypoint_paths(&graph, &entrypoints, &relevant);
        assert_eq!(
            paths["db::query"],
            vec!["entry:controller::index".to_string()]
        );
        assert_eq!(
            paths["service::run"],
            vec!["entry:controller::index".to_string()]
        );
    }

    #[test]
    fn score_finding_reflects_taint_and_reachability() {
        let mut violation = Violation {
            rule: "E0060".to_string(),
            line: "10".to_string(),
            message: crate::results::Message::new("E0060", "SQL injection"),
            start_line: 10,
            start_column: 1,
            end_line: 10,
            end_column: 20,
            taint_source: None,
            call_chain: vec![],
            meta: FindingMeta::from_rule("E0060", String::new()),
            generated_test: None,
        };
        let (score, reasons) = score_finding(&violation, true, true);
        assert!(score >= 50);
        assert!(reasons.iter().any(|r| r.contains("proven request taint")));
        assert!(reasons.iter().any(|r| r.contains("reachable")));
    }

    #[test]
    fn build_intelligence_includes_entrypoints_and_finding_context() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        let mut results = Results::default();
        let mut v = Violation {
            rule: "E0060".to_string(),
            line: "12".to_string(),
            message: crate::results::Message::new("E0060", "SQL injection"),
            start_line: 12,
            start_column: 1,
            end_line: 12,
            end_column: 40,
            taint_source: Some(crate::results::TaintSource {
                request_rooted: true,
                source_kind: Some("Request".to_string()),
                origins: vec!["$_GET".to_string()],
                file: Some("src/Controller.php".to_string()),
                line: Some(10),
                enclosing_function: Some("Controller::index".to_string()),
            }),
            call_chain: vec!["controller::index".to_string()],
            meta: FindingMeta::from_rule("E0060", String::new()),
            generated_test: None,
        };
        v.meta.fingerprint = "fp-1".to_string();
        results
            .files
            .insert("src/Controller.php".to_string(), vec![v]);

        let intel = analyse.build_intelligence(&results);
        assert!(intel.finding_context.contains_key("fp-1"));
        let ctx = &intel.finding_context["fp-1"];
        assert_eq!(ctx.rule, "E0060");
        assert_eq!(ctx.file, "src/Controller.php");
        assert!(ctx.reachable);
        assert!(ctx.risk_score > 0);
        assert!(ctx.data_classes.iter().any(|c| c == "request-input"));
        assert!(intel.nodes.iter().any(|n| n.kind == FlowNodeKind::Sink));
    }

    #[test]
    fn collect_php_files_skips_excluded_paths() {
        let base = std::env::temp_dir().join(format!("phanalist_excl_{}", std::process::id()));
        let included = base.join("src");
        let excluded = base.join("excluded");
        fs::create_dir_all(&included).unwrap();
        fs::create_dir_all(&excluded).unwrap();
        fs::write(included.join("Keep.php"), "<?php\n").unwrap();
        fs::write(excluded.join("Skip.php"), "<?php\n").unwrap();
        fs::write(included.join("Notes.txt"), "not php\n").unwrap();

        let found = collect_php_files(base.clone(), 0, None, &["**/excluded/*.php".to_string()]);

        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        fs::remove_dir_all(&base).ok();

        assert_eq!(names, vec!["Keep.php".to_string()]);
    }

    #[test]
    #[cfg(unix)]
    fn collect_php_files_handles_non_utf8_file_names() {
        // Regression guard for #214: the walk used to `unwrap()` the file name,
        // so a single file whose name is not valid UTF-8 panicked the walking
        // thread and left the run reporting a silently truncated result. Names
        // like this turn up easily under `vendor/`, `node_modules/` or build
        // output, which is exactly what `--src .` walks.
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let base = std::env::temp_dir().join(format!("phanalist_utf8_{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        fs::write(base.join("Plain.php"), "<?php\n").unwrap();

        // Not every filesystem can store such a name: macOS/APFS refuses with
        // `EILSEQ` ("Illegal byte sequence") instead of creating the file.
        // Writing it is therefore best-effort, and the assertions below are
        // stated in terms of what actually ended up on disk.
        let odd_name_created = fs::write(
            base.join(OsStr::from_bytes(b"Weird\xffName.php")),
            "<?php\n",
        )
        .is_ok();

        let found = collect_php_files(base.clone(), 0, None, &[]);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        fs::remove_dir_all(&base).ok();

        // The regression being guarded is that the walk aborted on such a name.
        // `Plain.php` must always be collected; the odd name is only expected
        // back where the filesystem accepted it.
        assert!(
            names.contains(&"Plain.php".to_string()),
            "the walk must not abort on a non-UTF-8 sibling: collected {names:?}"
        );
        if odd_name_created {
            assert_eq!(found.len(), 2, "both files must be collected: {names:?}");
        }
    }

    #[test]
    #[cfg(unix)]
    fn collect_php_files_follows_symlinked_files_but_not_linked_directories() {
        let base = std::env::temp_dir().join(format!("phanalist_links_{}", std::process::id()));
        let src = base.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("Real.php"), "<?php\n").unwrap();
        fs::write(base.join("Linked.php"), "<?php\n").unwrap();

        // A linked file must still be scanned...
        std::os::unix::fs::symlink(base.join("Linked.php"), src.join("Sym.php")).unwrap();
        // ...while a linked directory must not be descended into, matching
        // `follow_links(false)` and avoiding symlink loops.
        std::os::unix::fs::symlink(&src, base.join("loop")).unwrap();

        let mut names: Vec<String> = collect_php_files(base.clone(), 0, None, &[])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        names.sort();
        fs::remove_dir_all(&base).ok();

        assert!(names.contains(&"Real.php".to_string()));
        assert!(names.contains(&"Sym.php".to_string()));
        assert!(names.contains(&"Linked.php".to_string()));
    }

    #[test]
    fn scan_reports_files_without_leaking_previous_asts() {
        // Regression guard for #214: `scan` used to parse every file in the
        // project into one arena that was only freed at the end, so peak memory
        // grew with the size of the tree. Both passes now reset the arena per
        // file; the scan must still find every file exactly once.
        let base = std::env::temp_dir().join(format!("phanalist_scan_{}", std::process::id()));
        fs::create_dir_all(base.join("src")).unwrap();
        for i in 0..8 {
            fs::write(
                base.join("src").join(format!("Class{i}.php")),
                format!("<?php\nnamespace Scan;\nclass Class{i} {{ public function go(): void {{}} }}\n"),
            )
            .unwrap();
        }
        // Not a PHP file: must not be counted.
        fs::write(base.join("src").join("readme.md"), "hi\n").unwrap();

        let config = Config::default();
        let analyse = Analyse::new(&config);
        let results = analyse.scan(
            base.join("src").display().to_string(),
            &config,
            false,
            &Format::json,
            0,
            false,
            None,
        );

        fs::remove_dir_all(&base).ok();

        // Every PHP file is visited exactly once, and the non-PHP file is not
        // counted. `files` only holds files with something to report, so it is
        // not a reliable file count.
        assert_eq!(results.total_files_count, 8);
    }

    #[test]
    fn parallel_scan_matches_serial_counts() {
        let base = std::env::temp_dir().join(format!("phanalist_parallel_{}", std::process::id()));
        fs::create_dir_all(base.join("src")).unwrap();
        for i in 0..32 {
            fs::write(
                base.join("src").join(format!("Class{i}.php")),
                format!(
                    "<?php\nclass Foreign{i} {{ public function next(): Foreign{i} {{ return $this; }} }}\nclass Class{i} {{ public function run(Foreign{i} $f) {{ $f->next()->next(); }} }}\n"
                ),
            )
            .unwrap();
        }

        let config = Config::default();
        let run = |jobs: &str| {
            std::env::set_var("PHANALIST_JOBS", jobs);
            let analyse = Analyse::new(&config);
            let results = analyse.scan(
                base.join("src").display().to_string(),
                &config,
                false,
                &Format::json,
                0,
                false,
                None,
            );
            (results.total_files_count, results.codes_count)
        };

        let serial = run("1");
        let parallel = run("4");
        std::env::remove_var("PHANALIST_JOBS");
        fs::remove_dir_all(&base).ok();

        assert_eq!(serial.0, 32);
        assert_eq!(serial, parallel);
    }

    fn get_all_codes() -> Vec<String> {
        vec![
            "RULE1".to_string(),
            "RULE2".to_string(),
            "RULE3".to_string(),
            "RULE4".to_string(),
        ]
    }

    fn get_enabled_codes() -> Vec<String> {
        vec![
            "RULE1".to_string(),
            "RULE3".to_string(),
            "RULE103".to_string(),
        ]
    }

    fn get_disabled_codes() -> Vec<String> {
        vec![
            "RULE2".to_string(),
            "RULE3".to_string(),
            "RULE203".to_string(),
        ]
    }

    #[test]
    fn test_filter_active_codes_all_enabled() {
        let all_codes = get_all_codes();
        let active_codes = Analyse::filter_active_codes(all_codes.clone(), &[], &[]);

        assert_eq!(all_codes, active_codes);
    }

    #[test]
    fn test_filter_active_codes_some_enabled() {
        let all_codes = get_all_codes();
        let enabled_codes = get_enabled_codes();
        let active_codes = Analyse::filter_active_codes(all_codes, &enabled_codes, &[]);

        assert_eq!(vec!["RULE1".to_string(), "RULE3".to_string()], active_codes);
    }

    #[test]
    fn test_filter_active_codes_some_disabled() {
        let all_codes = get_all_codes();
        let disabled_codes = get_disabled_codes();
        let active_codes = Analyse::filter_active_codes(all_codes, &[], &disabled_codes);

        assert_eq!(vec!["RULE1".to_string(), "RULE4".to_string()], active_codes);
    }

    #[test]
    fn test_filter_active_codes_some_enabled_and_disabled() {
        let all_codes = get_all_codes();
        let disabled_codes = get_disabled_codes();
        let enabled_codes = get_enabled_codes();
        let active_codes = Analyse::filter_active_codes(all_codes, &enabled_codes, &disabled_codes);

        assert_eq!(vec!["RULE1".to_string()], active_codes);
    }
}
