extern crate exitcode;

use std::path::Path;
use std::process;
use std::str::FromStr;

use clap::{arg, Parser};

use crate::analyse::Analyse;
use crate::output::Format;

mod analyse;
mod config;
mod file;
mod output;
mod results;
mod rules;

///
/// A static analyser for your PHP project.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    #[arg(short, long, default_value = "./phanalist.yaml")]
    config: String,
    #[arg(short, long, default_value = "./src")]
    src: String,
    #[arg(short, long, default_value = "text")]
    /// Possible options: text, json
    output_format: String,
    #[arg(long)]
    /// Output only summary
    summary_only: bool,
    #[arg(short, long)]
    /// Do not output the results
    quiet: bool,
}

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    let args = Args::parse();

    let quiet = args.quiet;

    let path = args.src;
    if !Path::new(&path).exists() {
        println!("Path {} does not exist", path);
        process::exit(exitcode::IOERR);
    }

    let format = match output::Format::from_str(args.output_format.as_str()) {
        Ok(format) => format,
        Err(_) => {
            println!("Invalid input format");
            process::exit(exitcode::USAGE);
        }
    };

    let config = Analyse::parse_config(args.config, &format, quiet);
    let mut analyze = Analyse::new(&config);
    let mut results = analyze.scan(path, config, format != Format::json && !quiet);

    if !quiet {
        analyze.output(&mut results, format, args.summary_only);
    }

    if results.has_any_violations() {
        process::exit(exitcode::SOFTWARE);
    } else {
        process::exit(exitcode::OK);
    }
}
