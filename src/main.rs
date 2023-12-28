use clap::Parser;
use project::Project;
use std::path::PathBuf;

mod analyse;
mod language_server_protocol;
mod project;
mod rules;
mod storage;
///
/// A static analyser for your PHP project.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    /// The name of the directory where the files are located.
    #[arg(short, long, default_value = ".")]
    directory: String,
    /// Start the LSP server.
    #[arg(long)]
    deamon: bool,
}

/// .
fn main() {
    println!("Phanalist running. ");
    std::env::set_var("RUST_BACKTRACE", "1");
    let args = Args::parse();
    if args.deamon {
        language_server_protocol::start();
    } else {
        let path = PathBuf::from(args.directory);
        let mut project = Project::new(path);
        let now = std::time::Instant::now();
        let files = project.scan();
        project.run();
        println!("Analysed {} files in : {:.2?}", files, now.elapsed());
    }
}
