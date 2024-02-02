use clap::Parser;
use project::Project;
use std::path::{Path, PathBuf};

mod analyse;
mod config;
mod language_server_protocol;
mod project;
mod rules;
mod storage;

///
/// A static analyser for your PHP project.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(short, long, default_value = "./phanalist.yaml")]
    config: String,
    #[arg(short, long, default_value = "./src")]
    src: Option<String>,
    /// Start the LSP server.
    #[arg(long)]
    deamon: bool,
}

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    let args = Args::parse();
    if args.deamon {
        language_server_protocol::start();
    } else {
        let config = PathBuf::from(args.config);
        let mut project = Project::new(config, args.src);

        if !Path::new(project.config.src.as_str()).exists() {
            println!("Path {} does not exist", project.config.src);
            return;
        }

        let now = std::time::Instant::now();
        let files = project.scan();
        project.run();
        println!("Analysed {} files in : {:.2?}", files, now.elapsed());
    }
}
