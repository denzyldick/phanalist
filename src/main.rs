use clap::Parser;

use project::Project;
use std::path::PathBuf;

mod analyse;
mod project;
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
    let mut project = Project::new(path);
    let now = std::time::Instant::now();
    let files = project.scan();
    project.run();
    println!("Analysed {} files in : {:.2?}", files, now.elapsed());
}
