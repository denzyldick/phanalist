use std::str::FromStr;

use colored::Colorize;
use serde::{Deserialize, Serialize};

use cli_table::{format::Justify, Cell, Style, Table};

use crate::results::Results;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[allow(non_camel_case_types)]

pub enum Format {
    text,
    json,
}

impl FromStr for Format {
    type Err = ();

    fn from_str(input: &str) -> Result<Format, Self::Err> {
        match input {
            "text" => Ok(Format::text),
            "json" => Ok(Format::json),
            _ => Err(()),
        }
    }
}

pub(crate) trait OutputFormatter {
    fn output(_results: Results) {}
}

pub struct Text {}
impl OutputFormatter for Text {
    fn output(results: Results) {
        for (path, violations) in results.files.clone() {
            if !violations.is_empty() {
                let file_symbol = "--->".blue().bold();
                println!("{} {} ", file_symbol, path);
                println!(
                    "{} {}",
                    "Warnings detected: ".yellow().bold(),
                    violations.len().to_string().as_str().red().bold()
                );
                let line_symbol = "|".blue().bold();
                for suggestion in &violations {
                    println!(
                        "  {}:\t{}",
                        suggestion.rule.yellow().bold(),
                        suggestion.suggestion.bold()
                    );
                    println!(
                        "  {}\t{} {}",
                        format!("{}:{}", suggestion.span.line, suggestion.span.column)
                            .blue()
                            .bold(),
                        line_symbol,
                        suggestion.line
                    );
                    println!();
                }
                println!()
            }
        }

        let mut rows = vec![];
        for (rule_code, violations) in results.codes_count {
            rows.push(vec![
                rule_code.as_str().cell(),
                violations.cell().justify(Justify::Right),
            ]);
        }
        let table = rows
            .table()
            .title(vec![
                "Rule Code".cell().bold(true),
                "Violations".cell().bold(true),
            ])
            .bold(true);
        println!("{}", table.display().unwrap());

        println!(
            "Analysed {} files in : {:.2?}",
            results.total_files_count,
            results.duration.unwrap()
        );
    }
}

pub struct Json {}
impl OutputFormatter for Json {
    fn output(results: Results) {
        let mut output_results = results.clone();

        for (path, violations) in output_results.files.clone() {
            if violations.is_empty() {
                output_results.files.remove(&path);
            }
        }

        println!("{}", serde_json::to_string_pretty(&output_results).unwrap());
    }
}
