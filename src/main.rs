extern crate exitcode;
use colored::Colorize;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;

use clap::Parser;

use project::Project;

use crate::config::Config;
use crate::output::Format;

mod analyse;
mod config;
mod file;
mod output;
mod project;
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
    src: Option<String>,
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

    let output_format = output::Format::from_str(args.output_format.as_str());
    if output_format.is_err() {
        println!("Invalid input format");
        process::exit(exitcode::USAGE);
    }

    let format = output_format.unwrap();
    let quiet = args.quiet;
    let mut config = parse_config(args.config, &format, quiet);
    if let Some(src) = args.src {
        config.src = src;
    }

    if !Path::new(config.src.as_str()).exists() {
        println!("Path {} does not exist", config.src);
        process::exit(exitcode::IOERR);
    }

    let mut project = Project {};
    let show_scan_bar = format != Format::json && !quiet;
    let mut results = project.scan(config, show_scan_bar);

    if !quiet {
        project.output(&mut results, format, args.summary_only);
    }

    if results.has_any_violations() {
        process::exit(exitcode::SOFTWARE);
    } else {
        process::exit(exitcode::OK);
    }
}

fn parse_config(config_path: String, output_format: &Format, quiet: bool) -> Config {
    let path = PathBuf::from(config_path);
    let default_config = Config::default();

    let output_hints = !quiet && output_format != &Format::json;
    match fs::read_to_string(&path) {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            default_config.save(&path);

            if output_hints {
                println!(
                    "The new {} configuration file as been created",
                    &path.display().to_string().bold()
                );
            }

            default_config
        }

        Err(e) => {
            panic!("{}", e)
        }
        Ok(s) => {
            if output_hints {
                println!(
                    "Using configuration file {}",
                    &path.display().to_string().bold()
                );
            }

            match serde_yaml::from_str(&s) {
                Ok(c) => c,
                Err(e) => {
                    println!("Unable to use the config: {}. Ignoring it.", &e);
                    default_config
                }
            }
        }
    }
}
