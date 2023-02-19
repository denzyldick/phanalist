use clap::Parser;

use rocksdb::DB;
use rules::{File, Project};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{fs, thread};

mod analyse;
mod rules;
mod storage;

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
    let project = Project {
        files: Vec::new(),
        classes: HashMap::new(),
    };

    let now = std::time::Instant::now();
    thread::spawn(move || {
        rules::scan_folder(path, send);
    });

    let db = DB::open_default("/tmp").unwrap();
    let mut files = 0;
    for (content, path) in recv {
        for statement in rules::parse_code(content.as_str()).unwrap() {
            let file = &mut File {
                path: PathBuf::new(),
                ast: statement.clone(),
                members: Vec::new(),
                suggestions: Vec::new(),
            };
            storage::put(&db, path.display().to_string(), file.clone());
        }
        files = files + 1;
    }
    project.start(&db);
    println!("Analysed {} files in : {:.2?}", files, now.elapsed());
}
