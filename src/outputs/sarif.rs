use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde_sarif::sarif::{
    self, ArtifactLocation, Message, MultiformatMessageString, PhysicalLocation, Run,
    Sarif as StandardSarif, Tool, ToolComponent,
};

use crate::paths::normalize_relative;
use crate::results::{Results, Violation};
use crate::rules;

use super::OutputFormatter;
pub struct Sarif {}

const URI_PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'[')
    .add(b']')
    .add(b'^')
    .add(b'|');

fn markdown_to_text(markdown: &str) -> String {
    let mut text = Vec::new();
    let mut in_code_block = false;

    for line in markdown.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            text.push(line.trim_end().to_string());
            continue;
        }

        let line = line.trim_start().trim_start_matches('#').trim_start();
        text.push(
            line.replace("**", "")
                .replace('`', "")
                .trim_end()
                .to_string(),
        );
    }

    text.join("\n").trim().to_string()
}

fn encode_uri_path(path: &str) -> String {
    path.split('/')
        .map(|segment| utf8_percent_encode(segment, URI_PATH_SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn artifact_uri(path: &str) -> String {
    encode_uri_path(&normalize_relative(Path::new(path)))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn fingerprint(violation: &Violation, uri: &str) -> String {
    let input = format!(
        "{}\0{}\0{}\0{}\0{}",
        violation.rule, uri, violation.message.id, violation.start_line, violation.start_column
    );
    format!("{:016x}", fnv1a64(input.as_bytes()))
}

fn sarif_region(violation: &Violation) -> sarif::Region {
    let start_line = violation.start_line.max(1) as i64;
    let end_line = violation.end_line.max(violation.start_line.max(1)) as i64;
    let start_column = (violation.start_column as i64 + 1).max(1);
    let end_column = if end_line == start_line {
        (violation.end_column as i64 + 1).max(start_column)
    } else {
        (violation.end_column as i64 + 1).max(1)
    };

    sarif::Region {
        byte_length: None,
        byte_offset: None,
        char_length: None,
        char_offset: None,
        end_column: Some(end_column),
        end_line: Some(end_line),
        message: None,
        properties: None,
        snippet: None,
        source_language: Some("PHP".to_string()),
        start_column: Some(start_column),
        start_line: Some(start_line),
    }
}

impl OutputFormatter for Sarif {
    fn output(results: &mut Results) {
        let version: &str = env!("CARGO_PKG_VERSION");
        let description = MultiformatMessageString {
            markdown: None,
            properties: None,
            text: String::from(
                "Performant static analyzer for PHP, which is extremely easy to use. It helps you catch common mistakes in your PHP code.",
            ),
        };

        let mut all_rules: Vec<(String, Box<dyn rules::Rule>)> =
            rules::all_rules().into_iter().collect();
        all_rules.sort_by(|a, b| a.0.cmp(&b.0));

        let mut sarif_rules = vec![];
        let mut rule_to_index: HashMap<String, i64> = HashMap::new();
        for (current_index, (code, rule)) in all_rules.into_iter().enumerate() {
            let rule_number = code.trim_start_matches('E').trim_start_matches('0');
            let rule_number = if rule_number.is_empty() {
                "0"
            } else {
                rule_number
            };

            let detailed = rule.get_detailed_explanation();
            let text = detailed
                .as_deref()
                .map(markdown_to_text)
                .unwrap_or_else(|| rule.description());
            let full_message = MultiformatMessageString {
                markdown: detailed,
                properties: None,
                text,
            };
            let short_message = MultiformatMessageString {
                markdown: None,
                properties: None,
                text: rule.description(),
            };
            sarif_rules.push(sarif::ReportingDescriptor {
                default_configuration: None,
                deprecated_guids: None,
                deprecated_ids: None,
                deprecated_names: None,
                guid: None,
                help: Some(full_message.clone()),
                full_description: Some(full_message),
                help_uri: Some(format!(
                    "https://github.com/denzyldick/phanalist/blob/main/src/rules/examples/e{}/e{}.md",
                    rule_number, rule_number
                )),
                id: code.clone(),
                message_strings: None,
                name: Some(code.clone()),
                properties: None,
                relationships: None,
                short_description: Some(short_message),
            });
            rule_to_index.insert(code, current_index as i64);
        }
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
            rules: Some(sarif_rules),
            semantic_version: Some(version.to_string()),
            short_description: Some(description),
            supported_taxonomies: None,
            taxa: None,
            translation_metadata: None,
            version: Some(version.to_string()),
        };
        let tool = Tool {
            driver: tool_component,
            extensions: None,
            properties: None,
        };

        let mut files: Vec<(&String, &Vec<Violation>)> = results.files.iter().collect();
        files.sort_by(|a, b| a.0.cmp(b.0));

        let mut t = vec![];
        for (key, violations) in files {
            let uri = artifact_uri(key);
            let mut violations: Vec<&Violation> = violations.iter().collect();
            violations.sort_by(|a, b| {
                a.start_line
                    .cmp(&b.start_line)
                    .then(a.start_column.cmp(&b.start_column))
                    .then(a.end_line.cmp(&b.end_line))
                    .then(a.end_column.cmp(&b.end_column))
                    .then_with(|| a.rule.cmp(&b.rule))
                    .then_with(|| a.message.id.cmp(&b.message.id))
            });

            for violation in violations {
                let analysis_target = ArtifactLocation {
                    uri: Some(uri.clone()),
                    ..Default::default()
                };

                let message = Message {
                    text: Some(violation.message.render()),
                    ..Default::default()
                };

                let region = sarif_region(violation);

                let physical_location = PhysicalLocation {
                    address: None,
                    artifact_location: Some(analysis_target.clone()),
                    context_region: None,
                    properties: None,
                    region: Some(region),
                };

                let location = sarif::Location {
                    annotations: None,
                    id: None,
                    logical_locations: None,
                    message: None,
                    physical_location: Some(physical_location),
                    properties: None,
                    relationships: None,
                };

                let mut partial_fingerprints = BTreeMap::new();
                partial_fingerprints
                    .insert("phanalist/v1".to_string(), fingerprint(violation, &uri));

                t.push(serde_sarif::sarif::Result {
                    analysis_target: Some(analysis_target),
                    attachments: None,
                    baseline_state: None,
                    code_flows: None,
                    correlation_guid: None,
                    fingerprints: None,
                    fixes: None,
                    graph_traversals: None,
                    graphs: None,
                    guid: None,
                    hosted_viewer_uri: None,
                    kind: Some(serde_json::Value::String(String::from("fail"))),
                    level: Some(serde_json::Value::String(String::from("warning"))),
                    locations: Some(vec![location]),
                    message,
                    occurrence_count: None,
                    partial_fingerprints: Some(partial_fingerprints),
                    properties: None,
                    provenance: None,
                    rank: None,
                    related_locations: None,
                    rule: None,
                    rule_id: Some(violation.rule.clone()),
                    rule_index: rule_to_index.get(&violation.rule).copied(),
                    stacks: None,
                    suppressions: None,
                    taxa: None,
                    web_request: None,
                    web_response: None,
                    work_item_uris: None,
                });
            }
        }

        let runs = vec![Run {
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
            invocations: Some(vec![sarif::Invocation {
                account: None,
                arguments: None,
                command_line: None,
                end_time_utc: None,
                environment_variables: None,
                executable_location: None,
                execution_successful: true,
                exit_code: None,
                exit_code_description: None,
                exit_signal_name: None,
                exit_signal_number: None,
                machine: None,
                notification_configuration_overrides: None,
                process_id: None,
                process_start_failure_message: None,
                properties: None,
                response_files: None,
                rule_configuration_overrides: None,
                start_time_utc: None,
                stderr: None,
                stdin: None,
                stdout: None,
                stdout_stderr: None,
                tool_configuration_notifications: None,
                tool_execution_notifications: None,
                working_directory: None,
            }]),
            language: Some("en".to_string()),
            logical_locations: None,
            newline_sequences: None,
            original_uri_base_ids: None,
            policies: None,
            properties: None,
            redaction_tokens: None,
            results: Some(t),
            run_aggregates: None,
            special_locations: None,
            taxonomies: None,
            thread_flow_locations: None,
            tool,
            translations: None,
            version_control_provenance: None,
            web_requests: None,
            web_responses: None,
        }];
        let s = StandardSarif {
            schema: Some(String::from(
                "https://json.schemastore.org/sarif-2.1.0.json",
            )),
            inline_external_properties: None,
            properties: None,
            runs,
            version: serde_json::Value::String("2.1.0".to_string()),
        };
        let message = serde_json::json!(s);
        println!("{}", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::Message as DiagnosticMessage;

    fn violation(start_line: usize, start_column: usize, end_column: usize) -> Violation {
        Violation {
            rule: "E0035".to_string(),
            line: String::new(),
            message: DiagnosticMessage::new("e35.missing-key", "Missing array key"),
            start_line,
            start_column,
            end_line: start_line,
            end_column,
        }
    }

    #[test]
    fn converts_rule_markdown_to_readable_plain_text() {
        let markdown = "# Title\n\nUse `md5()` and **avoid** it.\n\n```php\nmd5($password);\n```\n";
        assert_eq!(
            markdown_to_text(markdown),
            "Title\n\nUse md5() and avoid it.\n\nmd5($password);"
        );
    }

    #[test]
    fn artifact_uri_is_relative_and_percent_encoded() {
        let cwd = std::env::current_dir().unwrap();
        let path = cwd.join("src/My File#1.php");
        assert_eq!(
            artifact_uri(path.to_str().unwrap()),
            "src/My%20File%231.php"
        );
    }

    #[test]
    fn artifact_uri_strips_leading_dot_slash() {
        assert_eq!(artifact_uri("./src/Foo.php"), "src/Foo.php");
    }

    #[test]
    fn region_columns_are_one_based() {
        let region = sarif_region(&violation(7, 11, 29));
        assert_eq!(region.start_line, Some(7));
        assert_eq!(region.start_column, Some(12));
        assert_eq!(region.end_line, Some(7));
        assert_eq!(region.end_column, Some(30));
    }

    #[test]
    fn fingerprint_is_stable_for_same_location() {
        let violation = violation(7, 11, 29);
        assert_eq!(
            fingerprint(&violation, "src/Foo.php"),
            fingerprint(&violation, "src/Foo.php")
        );
        assert_ne!(
            fingerprint(&violation, "src/Foo.php"),
            fingerprint(&violation, "src/Bar.php")
        );
    }
}
