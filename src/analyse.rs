use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Instant;

use colored::Colorize;
use indicatif::ProgressBar;
use jwalk::WalkDir;
use mago_allocator::prelude::LocalArena;
use mago_syntax::cst::Statement;

use crate::config::Config;
use crate::debug_stats::{FileTimings, RuleTimings};
use crate::file::File;
use crate::outputs::codeclimate::CodeClimate;
use crate::outputs::json::Json;
use crate::outputs::sarif::Sarif;
use crate::outputs::text::Text;
use crate::outputs::Format;
use crate::outputs::OutputFormatter;
use crate::results::{Results, Violation};
use crate::rules::Rule;
use crate::rules::{self};

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
}

impl Analyse {
    pub fn new(config: &Config) -> Self {
        Self {
            rules: Self::get_active_rules(config),
        }
    }

    #[allow(clippy::too_many_arguments)]
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
        let now = std::time::Instant::now();
        let mut results = Results::default();
        if collect_rule_metrics {
            results.rule_timings = Some(RuleTimings::default());
        }

        let has_external_bar = external_bar.is_some();

        if show_bar && format == &Format::text && !has_external_bar {
            println!();
            println!("Scanning files in {} ...", &path.to_string().bold());
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

        // One arena is reused for the whole scan, but reclaimed after every
        // file. `reset` frees each file's AST while keeping the chunks for
        // reuse, so the arena settles at the size of the largest single file
        // instead of growing to the size of the entire project.
        let mut arena = LocalArena::new();

        // 2. Pre-pass (indexing). Rules that resolve references across files
        //    need the whole project indexed before any file is validated, so
        //    every file is visited once up front. `index_file` implementations
        //    extract only lightweight summaries (class and member names,
        //    parents, namespaces), so nothing here needs the AST to outlive
        //    the file it came from.
        for file_path in &paths {
            if verbose >= 3 {
                log_line(
                    progress_bar.as_ref(),
                    format!("[vvv] indexing {}", file_path.display()),
                );
            }
            let Some(content) = read_source(file_path) else {
                continue;
            };

            {
                let file = File::new(&arena, file_path.clone(), content);
                for rule in self.rules.values() {
                    rule.index_file(&file);
                }
            }

            arena.reset();
        }

        // 3. Main pass. Each file is re-read and re-parsed, then dropped with
        //    the arena reset, so only one file's AST is live at a time. The
        //    re-read is served from the OS page cache and parsing is a fraction
        //    of the cost of validating, which buys the bounded memory profile.
        let mut files = 0;
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
                let (violations, file_timings) = self.analyse_file(&mut file, collect_rule_metrics);
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

        if bar_active && !has_external_bar {
            progress_bar.unwrap().finish();
        }

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
                            &path.display().to_string().bold(),
                            e
                        );
                    }
                } else if output_hints && output_format == &Format::text {
                    println!(
                        "The new {} configuration file as been created",
                        &path.display().to_string().bold()
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
                        &path.display().to_string().bold()
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
                            println!("Unable to use the config: {}. Ignoring it.", &e);
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
        // on a full-project scan is the largest allocation of the whole run.
        results.files.retain(|_, violations| !violations.is_empty());

        match format {
            Format::json => Json::output(results),
            Format::sarif => Sarif::output(results),
            Format::codeclimate => CodeClimate::output(results),
            _ => Text::output(results),
        };
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let odd_name_created = fs::write(base.join(OsStr::from_bytes(b"Weird\xffName.php")), "<?php\n").is_ok();

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
