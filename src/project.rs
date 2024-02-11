use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

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
    pub fn scan(&mut self, config: Config, show_bar: bool) -> Results {
        let now = std::time::Instant::now();
        let analyze = Analyse::new(&config);

        let mut results = Results::default();
        let progress_bar = self.get_progress_bar(&config.src);

        let (send, recv) = std::sync::mpsc::channel();
        let path = config.src;
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
