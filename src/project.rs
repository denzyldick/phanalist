use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use indicatif::ProgressBar;
use jwalk::WalkDir;
use php_parser_rs::parser;

use crate::analyse::Analyse;
use crate::config::Config;
use crate::file::File;
use crate::output::{Format, Json, OutputFormatter, Text};
use crate::results::Results;

pub struct Project {}

pub fn scan_folder(current_dir: PathBuf, sender: Sender<(String, PathBuf)>) {
    for entry in WalkDir::new(current_dir.clone()).follow_links(false) {
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
        let analyze = Analyse::new(config.clone());
        let mut results = Results::default();
        let files_count = WalkDir::new(config.src.clone())
            .follow_links(false)
            .into_iter()
            .count();
        let progress_bar = ProgressBar::new(files_count as u64);

        let (send, recv) = std::sync::mpsc::channel();
        let path = config.src.clone();
        std::thread::spawn(move || {
            let path = PathBuf::from(path);
            self::scan_folder(path, send);
        });

        let mut files = 0;
        for (content, path) in recv {
            if show_bar {
                progress_bar.inc(1);
            }

            let ast = match parser::parse(&content) {
                Ok(a) => a,
                Err(_) => vec![],
            };
            let file = File {
                content,
                path: path.clone(),
                ast: ast.clone(),
            };

            if file.get_fully_qualified_name().is_some() {
                for statement in file.ast.clone() {
                    results.add_file_violations(&file, analyze.analyse(&file, statement));
                }

                files += 1;
            };
        }

        if show_bar {
            progress_bar.finish();
        }

        results.total_files_count = files;
        results.duration = Some(now.elapsed());

        results
    }

    pub fn output(&mut self, mut results: Results, format: Format, summary_only: bool) {
        if summary_only {
            results.files = HashMap::new();
        };

        match format {
            Format::json => Json::output(results),
            _ => Text::output(results),
        };
    }
}
