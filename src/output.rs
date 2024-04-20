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

use serde_sarif::sarif::{Run, Sarif as StandardSarif, Tool, ToolComponent};
pub struct Sarif {}
impl OutputFormatter for Sarif {
    fn output(results: &mut Results) {
        let mut runs = vec![];

        let tool_component = ToolComponent{
            associated_component: todo!(),
            contents: todo!(),
            dotted_quad_file_version: todo!(),
            download_uri: todo!(),
            full_description: todo!(),
            full_name: Some("Phanalist".to_string()) ,
            global_message_strings: todo!(),
            guid: todo!(),
            information_uri: todo!(),
            is_comprehensive: todo!(),
            language: todo!(),
            localized_data_semantic_version: todo!(),
            locations: todo!(),
            minimum_required_localized_data_semantic_version: todo!(),
            name: todo!(),
            notifications: todo!(),
            organization: todo!(),
            product: todo!(),
            product_suite: todo!(),
            properties: todo!(),
            release_date_utc: todo!(),
            rules: todo!(),
            semantic_version: todo!(),
            short_description: todo!(),
            supported_taxonomies: todo!(),
            taxa: todo!(),
            translation_metadata: todo!(),
            version: todo!(),
        };
        let tool = Tool{
            driver:tool_component,
            extensions: None,
            properties: None,
        };
        runs.push(Run {
            addresses: todo!(),
            artifacts: todo!(),
            automation_details: todo!(),
            baseline_guid: todo!(),
            column_kind: todo!(),
            conversion: todo!(),
            default_encoding: todo!(),
            default_source_language: todo!(),
            external_property_file_references: todo!(),
            graphs: todo!(),
            invocations: todo!(),
            language: Some("en".to_string()),
            logical_locations: todo!(),
            newline_sequences: todo!(),
            original_uri_base_ids: todo!(),
            policies: todo!(),
            properties: todo!(),
            redaction_tokens: todo!(),
            results: todo!(),
            run_aggregates: todo!(),
            special_locations: todo!(),
            taxonomies: todo!(),
            thread_flow_locations: todo!(),
            tool: ,
            translations: todo!(),
            version_control_provenance: todo!(),
            web_requests: todo!(),
            web_responses: todo!(),
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
