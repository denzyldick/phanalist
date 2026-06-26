use std::error::Error;
use std::path::{Path, PathBuf};

use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    notification::{DidChangeTextDocument, DidOpenTextDocument, Notification as _, PublishDiagnostics},
    Diagnostic, DiagnosticSeverity, InitializeParams, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use mago_allocator::prelude::LocalArena;
use serde_json::Value;

use crate::analyse::Analyse;
use crate::config::Config;
use crate::file::File;

/// Start the LSP stdio server and listen for requests/notifications from the IDE client.
pub fn run_server(config: &Config) -> Result<(), Box<dyn Error>> {
    eprintln!("Starting Phanalist LSP server...");

    // Create stdio transport (the standard way LSP servers communicate with editors)
    let (connection, io_threads) = Connection::stdio();

    // Advertise our server capabilities to the editor.
    // We request FULL text document synchronization so that we always get the full content
    // of files on updates, making incremental parsing simple and extremely robust.
    let capabilities = ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::FULL,
        )),
        ..Default::default()
    };

    let server_capabilities = serde_json::to_value(&capabilities)?;
    let initialization_params = connection.initialize(server_capabilities)?;
    let params: InitializeParams = serde_json::from_value(initialization_params)?;

    // Instantiate static analyzer
    let analyse = Analyse::new(config);

    // Extract workspace root to index files on startup
    let workspace_root = params.root_uri
        .and_then(|uri| uri.to_file_path().ok())
        .or_else(|| params.root_path.map(PathBuf::from));

    if let Some(ref root) = workspace_root {
        eprintln!("Indexing workspace root: {}", root.display());
        index_workspace(&analyse, root, config);
        eprintln!("Workspace indexing complete.");
    }

    // Start message loop
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    break;
                }
                // Send a default OK response to other requests we don't explicitly handle yet
                let response = Response::new_ok(req.id, Value::Null);
                connection.sender.send(Message::Response(response))?;
            }
            Message::Response(_) => {}
            Message::Notification(not) => {
                match not.method.as_str() {
                    DidOpenTextDocument::METHOD => {
                        if let Ok(params) = serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(not.params) {
                            let uri = params.text_document.uri;
                            let text = params.text_document.text;
                            handle_document_update(&connection, &analyse, uri, text);
                        }
                    }
                    DidChangeTextDocument::METHOD => {
                        if let Ok(params) = serde_json::from_value::<lsp_types::DidChangeTextDocumentParams>(not.params) {
                            let uri = params.text_document.uri;
                            if let Some(change) = params.content_changes.into_iter().next() {
                                handle_document_update(&connection, &analyse, uri, change.text);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    io_threads.join()?;
    eprintln!("Phanalist LSP server stopped.");
    Ok(())
}

/// Index files in the workspace on startup to populate rule internal models (e.g. class hierarchies/extends)
fn index_workspace(analyse: &Analyse, root: &Path, config: &Config) {
    let (send, recv) = std::sync::mpsc::channel();
    let exclude_paths = config.exclude_paths.clone();
    crate::analyse::scan_folder(root.to_path_buf(), send, 0, None, exclude_paths);

    let arena = LocalArena::new();
    let mut count = 0;
    for (content, path) in recv {
        let file = File::new(&arena, path, content);
        for rule in analyse.rules.values() {
            rule.index_file(&file);
        }
        count += 1;
    }
    eprintln!("Indexed {} workspace files.", count);
}

/// Run analysis on the document's new text content and report diagnostics back to the IDE.
fn handle_document_update(
    connection: &Connection,
    analyse: &Analyse,
    uri: Url,
    text: String,
) {
    let diagnostics = match analyze_single_file(analyse, &uri, text) {
        Ok(diags) => diags,
        Err(err) => {
            eprintln!("Error analyzing file {}: {:?}", uri, err);
            return;
        }
    };

    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };

    let notification = Notification::new(
        PublishDiagnostics::METHOD.to_string(),
        params,
    );

    if let Err(err) = connection.sender.send(Message::Notification(notification)) {
        eprintln!("Failed to send diagnostics: {:?}", err);
    }
}

/// Build AST, run rules on the updated file content, and format results as LSP Diagnostics.
pub(crate) fn analyze_single_file(
    analyse: &Analyse,
    uri: &Url,
    content: String,
) -> Result<Vec<Diagnostic>, Box<dyn Error>> {
    let path = uri.to_file_path().map_err(|_| "Invalid file path")?;

    let arena = LocalArena::new();
    let mut file = File::new(&arena, path, content);

    // Re-run indexing for this file to update the rule indices with the latest content
    for rule in analyse.rules.values() {
        rule.index_file(&file);
    }

    // Run active static analysis rules
    let (violations, _) = analyse.analyse_file(&mut file, false);

    // Map Phanalist violations to LSP Diagnostics (0-indexed line and columns)
    let diagnostics: Vec<Diagnostic> = violations
        .into_iter()
        .map(|violation| {
            let start_line = violation.start_line.saturating_sub(1) as u32;
            let start_col = violation.start_column as u32;
            let end_line = violation.end_line.saturating_sub(1) as u32;
            let end_col = violation.end_column as u32;

            Diagnostic {
                range: Range::new(
                    Position::new(start_line, start_col),
                    Position::new(end_line, end_col),
                ),
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(lsp_types::NumberOrString::String(violation.rule)),
                code_description: None,
                source: Some("phanalist".to_string()),
                message: violation.message.render(),
                related_information: None,
                tags: None,
                data: None,
            }
        })
        .collect();

    Ok(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyse::Analyse;
    use crate::config::Config;
    use lsp_types::{DiagnosticSeverity, NumberOrString};

    fn make_uri(path: &str) -> Url {
        Url::parse(&format!("file://{path}")).unwrap()
    }

    #[test]
    fn clean_file_returns_no_diagnostics() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        let content = "<?php\n".to_string();

        let diagnostics = analyze_single_file(&analyse, &make_uri("/test.php"), content).unwrap();
        assert!(diagnostics.is_empty(), "expected no diagnostics for a minimal PHP file, got {diagnostics:?}");
    }

    #[test]
    fn file_with_too_many_params_triggers_e0007() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        let content = "<?php\n\nnamespace App;\n\nclass Demo {\n    public function foo($a, $b, $c, $d, $e, $f, $g, $h, $i) {}\n}\n".to_string();

        let diagnostics = analyze_single_file(&analyse, &make_uri("/test.php"), content).unwrap();
        assert!(!diagnostics.is_empty(), "expected at least one diagnostic for method with 9 parameters");

        let has_e0007 = diagnostics.iter().any(|d| {
            matches!(&d.code, Some(NumberOrString::String(code)) if code == "E0007")
        });
        assert!(has_e0007, "expected E0007 diagnostic among {diagnostics:?}");
    }

    #[test]
    fn diagnostics_use_warning_severity_and_phanalist_source() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        let content = "<?php\n\nnamespace App;\n\nclass Demo {\n    public function foo($a, $b, $c, $d, $e, $f, $g, $h, $i) {}\n}\n".to_string();

        let diagnostics = analyze_single_file(&analyse, &make_uri("/test.php"), content).unwrap();
        let d = diagnostics.iter().find(|d| {
            matches!(&d.code, Some(NumberOrString::String(code)) if code == "E0007")
        }).expect("expected E0007 diagnostic");

        assert_eq!(d.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(d.source.as_deref(), Some("phanalist"));
        assert!(d.message.contains("foo"), "message should mention the method name: {}", d.message);
        assert!(d.message.contains("8"), "message should mention the max parameter count: {}", d.message);
    }

    #[test]
    fn diagnostics_use_zero_indexed_lines() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        // Line 6 (1-based) has the method with too many params → 5 (0-based)
        let content = "<?php\n\nnamespace App;\n\nclass Demo {\n    public function foo($a, $b, $c, $d, $e, $f, $g, $h, $i) {}\n}\n".to_string();

        let diagnostics = analyze_single_file(&analyse, &make_uri("/test.php"), content).unwrap();
        let d = diagnostics.iter().find(|d| {
            matches!(&d.code, Some(NumberOrString::String(code)) if code == "E0007")
        }).expect("expected E0007 diagnostic");

        // The violation is on line 6 (1-based) → 5 (0-based)
        assert_eq!(d.range.start.line, 5, "expected 0-indexed line 5 for the method");
        assert_eq!(d.range.end.line, 5, "expected end line to match start line for a single-line method signature");
    }

    #[test]
    fn invalid_uri_returns_error() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        let uri = Url::parse("https://example.com/test.php").unwrap();
        let content = "<?php\n".to_string();

        let result = analyze_single_file(&analyse, &uri, content);
        assert!(result.is_err(), "expected error for non-file URI");
    }

    #[test]
    fn namespace_without_class_does_not_crash() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        let content = "<?php\n\nnamespace App;\n\nfunction helper() {}\n".to_string();

        let diagnostics = analyze_single_file(&analyse, &make_uri("/test.php"), content).unwrap();
        // Should not panic; may or may not have diagnostics depending on rules
        assert!(diagnostics.is_empty() || !diagnostics.is_empty());
    }

    #[test]
    fn invalid_php_syntax_is_handled_gracefully() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        let content = "<?php\n\nsyntax error !!! @@@\n".to_string();

        // Should not panic
        let result = analyze_single_file(&analyse, &make_uri("/test.php"), content);
        assert!(result.is_ok(), "should handle parse errors without crashing");
    }
}
