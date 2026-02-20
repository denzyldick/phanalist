use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use colored::Colorize;
use indicatif::ProgressBar;
use jwalk::WalkDir;
// use mago_ast::Program;
use mago_ast::Statement;

use crate::config::Config;
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

pub fn scan_folder(current_dir: PathBuf, sender: Sender<(String, PathBuf)>) {
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
    rules: HashMap<String, Box<dyn Rule>>,
}

impl Analyse {
    pub fn new(config: &Config) -> Self {
        Self {
            rules: Self::get_active_rules(config),
        }
    }
    pub(crate) fn scan(
        &self,
        path: String,
        _config: &Config,
        show_bar: bool,
        format: &Format,
    ) -> Results {
        let now = std::time::Instant::now();
        let mut results = Results::default();
        let progress_bar = self.get_progress_bar(&path);

        let (send, recv) = std::sync::mpsc::channel();

        if show_bar && format == &Format::text {
            println!();
            println!("Scanning files in {} ...", &path.to_string().bold());
        }

        std::thread::spawn(move || {
            let path = PathBuf::from(path);
            self::scan_folder(path, send);
        });

        // 1. Collect all files
        let mut scanned_files = Vec::new();
        for (content, path) in recv {
            scanned_files.push(File::new(path, content));
        }

        // 2. Pre-pass (indexing)
        for file in &scanned_files {
            for rule in self.rules.values() {
                rule.index_file(file);
            }
        }

        // 3. Main pass
        let mut files = 0;
        for mut file in scanned_files {
            if show_bar && format == &Format::text {
                progress_bar.inc(1);
            }

            let violations = self.analyse_file(&mut file);
            results.add_file_violations(&file, violations);

            files += 1;
        }

        if show_bar && format == &Format::text {
            progress_bar.finish();
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

                match serde_yaml::from_str(&s) {
                    Ok(c) => c,
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

    pub(crate) fn analyse_file(&self, file: &mut File) -> Vec<Violation> {
        let mut violations: Vec<Violation> = vec![];

        if let Some(program) = &file.ast {
            file.reference_counter.build_reference_counter(program);
            for statement in program.statements.iter() {
                violations.append(&mut self.analyse_file_statement(file, statement));
            }
        }
        violations
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

    pub fn analyse_file_statement(&self, file: &File, statement: &Statement) -> Vec<Violation> {
        let mut violations = Vec::new();

        for rule in self.rules.values() {
            if rule.do_validate(file) {
                for statement in rule.flatten_statements_to_validate(statement) {
                    violations.append(&mut rule.validate(file, statement));
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
