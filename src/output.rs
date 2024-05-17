use std::{collections::HashMap, str::FromStr};

use cli_table::{format::Justify, Cell, Style, Table};
use colored::Colorize;
use php_parser_rs::parser::ast::{arguments, properties};
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
    sarif,
}

impl FromStr for Format {
    type Err = ();

    fn from_str(input: &str) -> Result<Format, Self::Err> {
        match input {
            "text" => Ok(Format::text),
            "json" => Ok(Format::json),
            "sarif" => Ok(Format::sarif),
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

        let memory_usage = if let Some(usage) = memory_stats() {
            human_bytes(usage.physical_mem as f64)
        } else {
            "N/A".to_string()
        };

        println!(
            "Analysed {} files in {:.2?}, memory usage: {}",
            results.total_files_count,
            results.duration.unwrap(),
            memory_usage
        );
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

        if !rows.is_empty() {
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
}

pub struct Json {}
impl OutputFormatter for Json {
    fn output(results: &mut Results) {
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    }
}

use serde_sarif::sarif::{
    MultiformatMessageString, Run, Sarif as StandardSarif, Tool, ToolComponent,
};
pub struct Sarif {}
impl OutputFormatter for Sarif {
    fn output(results: &mut Results) {
        let mut runs = vec![];

        const VERSION: &str = env!("CARGO_PKG_VERSION");
        let description = MultiformatMessageString {
            markdown: None,
            properties: None,
            text: String::from("Performant static analyzer for PHP, which is extremely easy to use. It helps you catch common mistakes in your PHP code."),
        };
        let tool_component = ToolComponent {
            associated_component: None,
            contents: None,
            dotted_quad_file_version: None,
            download_uri: Some(String::from("https://github.com/denzyldick/phanalist")),
            full_description: Some(description.clone()),
            full_name: Some("Phanalist".to_string()),
            global_message_strings: None,
            guid: None,
            information_uri: Some(String::from("https://github.com/denzyldick/phanalist")),
            is_comprehensive: Some(false),
            language: Some(String::from("en")),
            localized_data_semantic_version: None,
            locations: None,
            minimum_required_localized_data_semantic_version: None,
            name: String::from("Phanalist"),
            notifications: None,
            organization: Some(String::from("https://denzyl.io")),
            product: None,
            product_suite: None,
            properties: None,
            release_date_utc: None,
            rules: None,
            semantic_version: Some(VERSION.to_string()),
            short_description: Some(description),
            supported_taxonomies: None,
            taxa: None,
            translation_metadata: None,
            version: Some(VERSION.to_string()),
        };
        let tool = Tool {
            driver: tool_component,
            extensions: None,
            properties: None,
        };
        runs.push(Run {
            addresses: None,
            artifacts: None,
            automation_details: None,
            baseline_guid: None,
            column_kind: None,
            conversion: None,
            default_encoding: None,
            default_source_language: Some(String::from("PHP")),
            external_property_file_references: None,
            graphs: None,
            invocations: None,
            language: Some("en".to_string()),
            logical_locations: None,
            newline_sequences: None,
            original_uri_base_ids: None,
            policies: None,
            properties: None,
            redaction_tokens: None,
            results: None,
            run_aggregates: None,
            special_locations: None,
            taxonomies: None,
            thread_flow_locations: None,
            tool,
            translations: None,
            version_control_provenance: None,
            web_requests: None,
            web_responses: None,
        });

        let s = StandardSarif {
            schema: None,
            inline_external_properties: None,
            properties: None,
            runs,
            version: serde_json::Value::String("v0.1.21".to_string()),
        };
        let message = serde_json::json!(s);
        println!("{}", message);
    }
}
