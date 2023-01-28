use clap::Parser;
use php_parser_rs::parser;
use php_parser_rs::parser::ast::classes::ClassMember;
use std::io::Result;
use std::path::PathBuf;
use std::{env, fs};
mod rules;


/// A static analyser for your PHP project.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about=None)]
struct Args {
    /// The name of the directory where the files are located.
    #[arg(short, long)]
    directory: String,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let path = PathBuf::from(args.directory);
    scan_folder(path).unwrap();
    Ok(())
}

fn scan_folder(current_dir: PathBuf) -> Result<()> {
    for entry in fs::read_dir(current_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let metadata = fs::metadata(&path).unwrap();

        if metadata.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "php" {
                    let content = fs::read_to_string(entry.path());
                    match content {
                        Err(err) => {
                            println!("{:?}", err);
                        }
                        Ok(content) => {
                            for statement in parse_code(content.as_str()) {
                                let mut file = rules::File {
                                    path: entry.path(),
                                    ast: Some(statement),
                                    members: Vec::new(),
                                    suggestions: Vec::new(),
                                };
                                file.start();
                                file.output(rules::Output::STDOUT);
                            }
                        }
                    }
                }
            }
        } else if metadata.is_dir() {
            scan_folder(path).unwrap()
        }
    }
    Ok(())
}

#[warn(unreachable_code)]
fn parse_code(code: &str) -> Vec<php_parser_rs::parser::ast::Statement> {
    match parser::parse(code) {
        Ok(ast) => ast,
        Err(err) => {
            // println!("{:?}", err);
            vec![]
        }
    }
}
