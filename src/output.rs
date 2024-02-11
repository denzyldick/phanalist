use std::str::FromStr;

use cli_table::{format::Justify, Cell, Style, Table};
use colored::Colorize;
use serde::{Deserialize, Serialize};

use human_bytes::human_bytes;
use memory_stats::memory_stats;

use crate::results::Results;
use crate::rules;

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
    fn output(_results: &mut Results) {}
}

pub struct Text {}
impl OutputFormatter for Text {
    fn output(results: &mut Results) {
        Self::output_files_with_violations(results);
        Self::output_summary(results);

        println!(
            "Analysed {} files in : {:.2?}",
            results.total_files_count,
            results.duration.unwrap()
        );

        if let Some(usage) = memory_stats() {
            println!("Memory usage: {}", human_bytes(usage.physical_mem as f64));
        }
    }
}

impl Text {
    fn output_files_with_violations(results: &Results) {
        for (path, violations) in &results.files {
            if !violations.is_empty() {
                println!(
                    "{}, detected {} violations:",
                    path.blue().bold(),
                    violations.len().to_string().as_str().red().bold()
                );
                let line_symbol = "|".blue().bold();
                for suggestion in violations {
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
                }
                println!()
            }
        }
    }

    fn output_summary(results: &Results) {
        let all_rules = rules::all_rules();
        let mut rows = vec![];

        let mut sorted_codes_count = results.codes_count.clone().into_iter().collect::<Vec<_>>();
        sorted_codes_count.sort_by(|a, b| b.1.cmp(&a.1));
        for (rule_code, violations) in sorted_codes_count {
            let rule = all_rules.get(&rule_code).unwrap();

            rows.push(vec![
                rule_code.as_str().cell(),
                rule.description().cell(),
                violations.cell().justify(Justify::Right),
            ]);
        }

        let table = rows
            .table()
            .title(vec![
                "Rule Code".cell().bold(true),
                "Description".cell().bold(true),
                "Violations".cell().bold(true),
            ])
            .bold(true);
        println!("{}", table.display().unwrap());
    }
}

pub struct Json {}
impl OutputFormatter for Json {
    fn output(results: &mut Results) {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    }
}
