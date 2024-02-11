extern crate exitcode;

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
    #[arg(short, long)]
    default_config: bool,
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
    let mut config = if args.default_config {
        Config::default()
    } else {
        parse_config(args.config, quiet)
    };
    if let Some(src) = args.src {
        config.src = src;
    }

    if !Path::new(config.src.as_str()).exists() {
        println!("Path {} does not exist", config.src);
        process::exit(exitcode::IOERR);
    }

    let mut project = Project {};
    let is_not_json_format = format != Format::json;
    if is_not_json_format && !quiet {
        println!();
        println!("Scanning files ...");
    }
    let mut results = project.scan(config, is_not_json_format && !quiet);

    if !quiet {
        project.output(&mut results, format, args.summary_only);
    }

    if results.has_any_violations() {
        process::exit(exitcode::SOFTWARE);
    } else {
        process::exit(exitcode::OK);
    }
}

fn parse_config(config_path: String, quiet: bool) -> Config {
    let path = PathBuf::from(config_path);
    let default_config = Config::default();

    match fs::read_to_string(&path) {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            println!("No configuration file {} has been found.", &path.display());
            println!("Do you want to create a configuration file (otherwise defaults will be used)? [Y/n]");

            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer).unwrap();

            if answer.trim().to_lowercase() == "y" || answer.trim().to_lowercase() == "yes" {
                default_config.save(&path);
                println!(
                    "The new {} configuration file as been created",
                    &path.display()
                );
            };

            default_config
        }

        Err(e) => {
            panic!("{}", e)
        }
        Ok(s) => {
            if !quiet {
                println!("Using configuration file {}", &path.display());
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
