use clap::Parser;
use php_parser_rs::parser::ast::classes::ClassMember;
use rules::Project;
use std::collections::HashMap;
use std::io::Result;
use std::path::PathBuf;
use std::{env, fs};

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
fn main() -> Result<()> {
    let args = Args::parse();
    let path = PathBuf::from(args.directory);
    let mut project = Project {
        files: Vec::new(),
        classes: HashMap::new(),
    };

    project.scan_folder(path);
    project.start()?;
    Ok(())
}
