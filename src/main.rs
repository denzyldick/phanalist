use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::Parser;

use project::Project;

use crate::config::Config;
use crate::output::Format;

mod analyse;
mod config;
mod file;
mod language_server_protocol;
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
    output_summary_only: bool,
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
        let output_format = output::Format::from_str(args.output_format.as_str());
        if output_format.is_err() {
            println!("Invalid input format");
            return;
        }

        let format = output_format.clone().unwrap();
        let mut config = parse_config(PathBuf::from(args.config), format.clone());
        if let Some(src) = args.src {
            config.src = src;
        }

        if !Path::new(config.src.as_str()).exists() {
            println!("Path {} does not exist", config.src);
            return;
        }

        let mut project = Project::new(config);

        project.scan();
        project.output(output_format.unwrap(), args.output_summary_only);
    }
}

fn parse_config(path: PathBuf, format: Format) -> Config {
    let default_config = Config::default();

    match fs::read_to_string(path.clone()) {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            println!(
                "No configuration file {} has been found.",
                &path.clone().display()
            );
            println!("Do you want to create a configuration file (otherwise defaults will be used)? [Y/n]");

            let mut answer = String::new();
            std::io::stdin().read_line(&mut answer).unwrap();

            if answer.trim().to_lowercase() == "y" || answer.trim().to_lowercase() == "yes" {
                default_config.save(path.clone());
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
            if format == Format::text {
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
