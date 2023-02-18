use clap::Parser;
use php_parser_rs::parser::ast::classes::ClassMember;
use rules::{File, Output, Project};
use std::collections::HashMap;
use std::io::Result;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{env, fs, thread};

mod analyse;
mod rules;

/// A static analyser for your PHP project.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    /// The name of the directory where the files are located.
    #[arg(short, long, default_value = ".")]
    directory: String,
}

/// .
///
/// # Errors
///
/// This function will return an error if .
fn main() {
    let args = Args::parse();
    let path = PathBuf::from(args.directory);
    let (send, recv) = mpsc::channel();
    let project = &mut Project {
        files: Vec::new(),
        classes: HashMap::new(),
    };

    let now = std::time::Instant::now();
    let handle = thread::spawn(move || {
        rules::scan_folder(path, send);
    });

    let mut files = 0;
    for (content, path) in recv {
        for statement in rules::parse_code(content.as_str()).unwrap() {
            let file = &mut File {
                path: PathBuf::new(),
                ast: Some(statement.clone()),
                members: Vec::new(),
                suggestions: Vec::new(),
            };
            project.analyze(file);
        }
        files = files + 1;
    }
    println!(
        "Analysed {} files in : {:.2?}",
        files,
        now.elapsed()
    );
}
