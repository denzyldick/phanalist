use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::Instant;

use bumpalo::Bump;
use colored::Colorize;
use indicatif::ProgressBar;
use jwalk::WalkDir;
use mago_syntax::ast::Statement;

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

pub fn scan_folder(
    current_dir: PathBuf,
    sender: Sender<(String, PathBuf)>,
    verbose: u8,
    bar: Option<ProgressBar>,
    exclude_paths: Vec<String>,
) {
    for entry in WalkDir::new(current_dir).follow_links(false) {
        let entry = entry.unwrap();
        let path = entry.path();
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let file_name = match path.file_name() {
            Some(f) => String::from(f.to_str().unwrap()),
            None => String::from(""),
        };
        if (file_name != "." || !file_name.is_empty()) && metadata.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "php" {
                    if !exclude_paths.is_empty()
                        && crate::paths::is_excluded(
                            &crate::paths::normalize_relative(&path),
                            &exclude_paths,
                        )
                    {
                        if verbose >= 2 {
                            log_line(bar.as_ref(), format!("[vv] excluded {}", path.display()));
                        }
                        continue;
                    }
                    if verbose >= 2 {
                        log_line(bar.as_ref(), format!("[vv] reading {}", path.display()));
                    }
                    let content = fs::read_to_string(entry.path());
                    match content {
                        Err(_) => {
                            // println!("{err:?}");
                        }
                        Ok(content) => {
                            sender.send((content, path)).unwrap();
                        }
                    }
                }
            }
        }
    }
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
        let progress_bar = if let Some(pb) = external_bar {
            Some(pb)
        } else if show_bar && format == &Format::text {
            Some(self.get_progress_bar(&path))
        } else {
            None
        };

        let (send, recv) = std::sync::mpsc::channel();

        let bar_active = progress_bar.is_some();
        let thread_bar = progress_bar.clone();

        if show_bar && format == &Format::text && !has_external_bar {
            println!();
            println!("Scanning files in {} ...", &path.to_string().bold());
        }

        if verbose >= 1 {
            for pattern in crate::paths::missing_literal_excludes(&config.exclude_paths) {
                log_line(
                    thread_bar.as_ref(),
                    format!("exclude_paths: '{pattern}' matches no existing path — typo?")
                        .yellow()
                        .to_string(),
                );
            }
        }

        let scan_path = path.clone();
        let exclude_paths = config.exclude_paths.clone();
        std::thread::spawn(move || {
            let path = PathBuf::from(scan_path);
            self::scan_folder(path, send, verbose, thread_bar, exclude_paths);
        });

        let arena = Bump::new();

        // 1. Collect all files
        let mut scanned_files: Vec<File<'_>> = Vec::new();
        for (content, path) in recv {
            if verbose >= 2 {
                log_line(
                    progress_bar.as_ref(),
                    format!("[vv] parsing {}", path.display()),
                );
            }
            scanned_files.push(File::new(&arena, path, content));
        }

        // 2. Pre-pass (indexing).
        for file in &scanned_files {
            if verbose >= 3 {
                log_line(
                    progress_bar.as_ref(),
                    format!("[vvv] indexing {}", file.path.display()),
                );
            }
            for rule in self.rules.values() {
                rule.index_file(file);
            }
        }

        // 3. Main pass.
        let mut files = 0;
        for mut file in scanned_files {
            if verbose >= 1 {
                log_line(
                    progress_bar.as_ref(),
                    format!("[v] analysing {}", file.path.display()),
                );
            }
            if let Some(ref pb) = progress_bar {
                pb.inc(1);
            }

            let (violations, file_timings) = self.analyse_file(&mut file, collect_rule_metrics);
            let file_path = file.path.display().to_string();
            results.add_file_violations(&file, violations);

            if let (Some(rt), Some(ft)) = (results.rule_timings.as_mut(), file_timings) {
                rt.merge_file(file_path, ft);
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

        for (path, violations) in results.files.clone() {
            if violations.is_empty() {
                results.files.remove(&path);
            }
        }

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

    fn get_progress_bar(&self, src_path: &str) -> ProgressBar {
        let files_count = WalkDir::new(src_path)
            .follow_links(false)
            .into_iter()
            .count();

        ProgressBar::new(files_count as u64)
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
    use std::sync::mpsc::channel;

    #[test]
    fn scan_folder_skips_excluded_paths() {
        let base =
            std::env::temp_dir().join(format!("phanalist_excl_{}", std::process::id()));
        let included = base.join("src");
        let excluded = base.join("excluded");
        fs::create_dir_all(&included).unwrap();
        fs::create_dir_all(&excluded).unwrap();
        fs::write(included.join("Keep.php"), "<?php\n").unwrap();
        fs::write(excluded.join("Skip.php"), "<?php\n").unwrap();

        let (send, recv) = channel();
        scan_folder(base.clone(), send, 0, None, vec!["**/excluded/*.php".to_string()]);

        let names: Vec<String> = recv
            .iter()
            .map(|(_, p)| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        fs::remove_dir_all(&base).ok();

        assert!(names.contains(&"Keep.php".to_string()));
        assert!(!names.contains(&"Skip.php".to_string()));
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
