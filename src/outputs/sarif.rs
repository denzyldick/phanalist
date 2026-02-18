use serde_sarif::sarif::{
    self, ArtifactLocation, Message, MultiformatMessageString, PhysicalLocation, Run,
    Sarif as StandardSarif, Tool, ToolComponent,
};

use crate::{results::Results, rules};

use super::OutputFormatter;
pub struct Sarif {}
impl OutputFormatter for Sarif {
    fn output(results: &mut Results) {
        const VERSION: &str = "v0.1.21";
        let description = MultiformatMessageString {
            markdown: None,
            properties: None,
            text: String::from("Performant static analyzer for PHP, which is extremely easy to use. It helps you catch common mistakes in your PHP code."),
        };

        let mut sarif_rules = vec![];
        let rules = rules::all_rules();
        for rule in rules {
            let r = rule.1.description();
            let description = r;

            let multiformat_message = MultiformatMessageString {
                markdown: rule.1.get_detailed_explanation(),
                properties: None,
                text: description,
            };
            sarif_rules.push(sarif::ReportingDescriptor {
                default_configuration: None,
                deprecated_guids: None,
                deprecated_ids: None,
                deprecated_names: None,
                guid: None,
                help: Some(multiformat_message.clone()),
                full_description: Some(multiformat_message.clone()),
                help_uri: None,
                id: rule.0.clone(),
                message_strings: None,
                name: Some(rule.0),
                properties: None,
                relationships: None,
                short_description: Some(multiformat_message),
            });
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

        let mut t = vec![];
        for (key, violations) in &results.files {
            for violation in violations {
                let analysis_target = ArtifactLocation {
                    uri: Some(String::from(key).replace("./", "")),
                    ..Default::default()
                };

                let message = Message {
                    text: Some(String::from(&violation.suggestion)),
                    ..Default::default()
                };

                let region = sarif::Region {
                    byte_length: None,
                    byte_offset: None,
                    char_length: None,
                    char_offset: None,
                    end_column: Some((violation.end_column as i64).max(1)),
                    end_line: Some((violation.end_line as i64).max(1)),
                    message: None,
                    properties: None,
                    snippet: None,
                    source_language: Some("PHP".to_string()),
                    start_column: Some((violation.start_column as i64).max(1)),
                    start_line: Some((violation.start_line as i64).max(1)),
                };

                let physical_location = PhysicalLocation {
                    address: None,
                    artifact_location: Some(analysis_target.clone()),
                    context_region: None,
                    properties: None,
                    region: Some(region.clone()),
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
                    kind: None,
                    level: None,
                    locations: Some(vec![location]),
                    message,
                    occurrence_count: None,
                    partial_fingerprints: None,
                    properties: None,
                    provenance: None,
                    rank: None,
                    related_locations: None,
                    rule: None,
                    rule_id: Some(violation.rule.clone()),
                    rule_index: None,
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
            invocations: None,
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
            schema: Some(String::from("https://json.schemastore.org/sarif-2.1.0")),
            inline_external_properties: None,
            properties: None,
            runs,
            version: serde_json::Value::String("2.1.0".to_string()),
        };
        let message = serde_json::json!(s);
        println!("{}", message);
    }
}
