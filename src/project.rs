use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use colored::Colorize;
use indicatif::ProgressBar;
use jwalk::WalkDir;

use crate::analyse::Analyse;
use crate::config::Config;
use crate::file::File;
use crate::output::{Format, Json, OutputFormatter, Text};
use crate::results::{Results, Violation};

pub struct Project {}

pub fn scan_folder(current_dir: PathBuf, sender: Sender<(String, PathBuf)>) {
    for entry in WalkDir::new(current_dir).follow_links(false) {
        let entry = entry.unwrap();
        let path = entry.path();
        let metadata = fs::metadata(&path).unwrap();
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

impl Project {
    pub(crate) fn scan(&self, path: String, config: Config, show_bar: bool) -> Results {
        let now = std::time::Instant::now();
        let analyze = Analyse::new(&config);

        let mut results = Results::default();
        let progress_bar = self.get_progress_bar(&path);

        let (send, recv) = std::sync::mpsc::channel();

        if show_bar {
            println!();
            println!("Scanning files in {} ...", &path.to_string().bold());
        }

        std::thread::spawn(move || {
            let path = PathBuf::from(path);
            self::scan_folder(path, send);
        });

        let mut files = 0;
        for (content, path) in recv {
            if show_bar {
                progress_bar.inc(1);
            }

            let file = File::new(path, content);
            let violations = self.analyse_file(&file, &analyze);
            results.add_file_violations(&file, violations);

            files += 1;
        }

        if show_bar {
            progress_bar.finish();
        }

        results.total_files_count = files;
        results.duration = Some(now.elapsed());

        results
    }

    pub(crate) fn parse_config(
        &self,
        config_path: String,
        output_format: &Format,
        quiet: bool,
    ) -> Config {
        let path = PathBuf::from(config_path);
        let default_config = Config::default();

        let output_hints = !quiet && output_format != &Format::json;
        match fs::read_to_string(&path) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                if let Err(e) = default_config.save(&path) {
                    println!(
                        "Unable to save {} configuration file, error: {}",
                        &path.display().to_string().bold(),
                        e
                    );
                } else if output_hints {
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
                if output_hints {
                    println!(
                        "Using configuration file {}",
                        &path.display().to_string().bold()
                    );
                }

                match serde_yaml::from_str(&s) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("Unable to use the config: {}. Ignoring it.", &e);
                        default_config
                    }
                }
            }
        }
    }

    fn get_progress_bar(&self, src_path: &str) -> ProgressBar {
        let files_count = WalkDir::new(src_path)
            .follow_links(false)
            .into_iter()
            .count();

        ProgressBar::new(files_count as u64)
    }

    pub fn output(&mut self, results: &mut Results, format: Format, summary_only: bool) {
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
            _ => Text::output(results),
        };
    }

    pub(crate) fn analyse_file(&self, file: &File, analyze: &Analyse) -> Vec<Violation> {
        let mut violations: Vec<Violation> = vec![];

        if file.get_fully_qualified_name().is_some() {
            for statement in &file.ast {
                violations.append(&mut analyze.analyse(file, statement));
            }
        };

        violations
    }
}
