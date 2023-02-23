use clap::Parser;

use rocksdb::DB;
use rules::{File, Project};
use std::collections::{HashMap, HashSet};
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

fn main() {
    let args = Args::parse();
    let path = PathBuf::from(args.directory);
    let (send, recv) = mpsc::channel();
    let mut project = Project {
        files: Vec::new(),
        classes: HashMap::new(),
    };

    let now = std::time::Instant::now();
    thread::spawn(move || {
        rules::scan_folder(path, send);
    });

    let file_path = "/tmp/phanalist";
    let file = std::path::Path::new(file_path);

    if file.is_dir() {
        match fs::remove_dir_all(file) {
            Ok(_) => {}
            Err(error) => {}
        }
    }

    let db = DB::open_default("/tmp/phanalist").unwrap();
    let mut files = 0;
    for (content, path) in recv {
        let file = &mut File {
            content: content,
            path: path.clone(),
            ast: Vec::new(),
            members: Vec::new(),
            suggestions: Vec::new(),
        };
        storage::put(&db, path.display().to_string(), file.clone());

        files = files + 1;
    }
    project.run(&db);
    println!("Analysed {} files in : {:.2?}", files, now.elapsed());
}
