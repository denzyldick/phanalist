use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use jwalk::WalkDir;
use php_parser_rs::parser;
use php_parser_rs::parser::ast::classes::ClassStatement;

use crate::analyse::Analyse;
use crate::config::Config;
use crate::file::File;
use crate::output::{Format, Json, OutputFormatter, Text};
use crate::results::Results;

pub struct Project {
    pub files: Vec<File>,
    pub classes: HashMap<String, ClassStatement>,
    pub config: Config,
    analyse: Analyse,
    results: Results,
}

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
    pub fn new(config: Config) -> Self {
        Self {
            files: Vec::new(),
            classes: HashMap::new(),
            config: config.clone(),
            analyse: Analyse::new(config.clone()),
            results: Results::default(),
        }
    }

    pub fn scan(&mut self) {
        let now = std::time::Instant::now();

        let (send, recv) = std::sync::mpsc::channel();
        let path = self.config.src.clone();
        std::thread::spawn(move || {
            let path = PathBuf::from(path);
            self::scan_folder(path, send);
        });

        let mut files = 0;
        for (content, path) in recv {
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
                    self.results
                        .add_file_violations(&file, self.analyse.analyse(&file, statement));
                }

                files += 1;
            };
        }

        self.results.total_files_count = files;
        self.results.duration = Some(now.elapsed());
    }

    pub fn output(&mut self, format: Format, summary_only: bool) {
        let mut output_results = self.results.clone();
        if summary_only {
            output_results.files = HashMap::new();
        };

        match format {
            Format::json => Json::output(output_results),
            _ => Text::output(output_results),
        };
    }
}
