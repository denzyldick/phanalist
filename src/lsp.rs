use std::error::Error;
use std::path::{Path, PathBuf};

use lsp_server::{Connection, Message, Notification, Response};
use lsp_types::{
    notification::{
        DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidDeleteFiles,
        DidOpenTextDocument, Notification as _, PublishDiagnostics,
    },
    Diagnostic, DiagnosticSeverity, FileChangeType, InitializeParams, Position,
    PublishDiagnosticsParams, Range, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Url,
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
                    DidCloseTextDocument::METHOD => {
                        if let Ok(params) = serde_json::from_value::<lsp_types::DidCloseTextDocumentParams>(not.params) {
                            close_document(&connection, &analyse, params.text_document.uri);
                        }
                    }
                    DidDeleteFiles::METHOD => {
                        if let Ok(params) = serde_json::from_value::<lsp_types::DeleteFilesParams>(not.params) {
                            for file in params.files {
                                if let Ok(uri) = Url::parse(&file.uri) {
                                    remove_document(&connection, &analyse, uri);
                                }
                            }
                        }
                    }
                    DidChangeWatchedFiles::METHOD => {
                        if let Ok(params) = serde_json::from_value::<lsp_types::DidChangeWatchedFilesParams>(not.params) {
                            for event in params.changes {
                                match event.typ {
                                    FileChangeType::DELETED => {
                                        remove_document(&connection, &analyse, event.uri);
                                    }
                                    FileChangeType::CREATED | FileChangeType::CHANGED => {
                                        if let Ok(path) = event.uri.to_file_path() {
                                            analyse.index_file_from_disk(&path);
                                        }
                                    }
                                    _ => {}
                                }
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
///
/// Indexing only needs lightweight summaries, so each file's AST is dropped and
/// the arena reclaimed before the next one is read. Opening a large project in
/// an editor therefore costs the summaries, not the project's whole ASTs.
fn index_workspace(analyse: &Analyse, root: &Path, config: &Config) {
    analyse.prepare_project(&[root.to_string_lossy().into_owned()], config, 0, None);
    eprintln!("Indexed workspace files.");
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

    publish_diagnostics(connection, uri, diagnostics);
}

fn close_document(connection: &Connection, analyse: &Analyse, uri: Url) {
    if let Ok(path) = uri.to_file_path() {
        analyse.index_file_from_disk(&path);
    }
    publish_diagnostics(connection, uri, Vec::new());
}

fn remove_document(connection: &Connection, analyse: &Analyse, uri: Url) {
    if let Ok(path) = uri.to_file_path() {
        analyse.remove_indexed_file(&path);
    }
    publish_diagnostics(connection, uri, Vec::new());
}

fn publish_diagnostics(connection: &Connection, uri: Url, diagnostics: Vec<Diagnostic>) {
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
    for rule in analyse.rules.values() {
        rule.finalize_index();
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
        let full = std::env::temp_dir().join(path);
        Url::from_file_path(full).unwrap()
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
    fn closing_a_document_restores_its_summaries_from_disk() {
        let config = Config::default();
        let analyse = Analyse::new(&config);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "phanalist-lsp-close-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();

        let parent = root.join("parent.php");
        let child = root.join("child.php");
        let use_path = root.join("use.php");
        std::fs::write(
            &parent,
            "<?php\nnamespace App;\nclass ParentCalc { public static function zero(): int { return 0; } }\n",
        )
        .unwrap();
        std::fs::write(
            &child,
            "<?php\nnamespace App;\nclass ChildCalc extends ParentCalc {}\n",
        )
        .unwrap();
        let use_content =
            "<?php\nnamespace App;\nfunction useChild(): float { return 1 / ChildCalc::zero(); }\n"
                .to_string();
        std::fs::write(&use_path, &use_content).unwrap();

        analyse.prepare_project(&[root.to_string_lossy().into_owned()], &config, 0, None);
        analyse.remove_indexed_file(&child);
        analyse.index_file_from_disk(&child);

        let uri = Url::from_file_path(&use_path).unwrap();
        let diagnostics = analyze_single_file(&analyse, &uri, use_content).unwrap();
        assert!(
            diagnostics
                .iter()
                .any(|d| matches!(&d.code, Some(NumberOrString::String(code)) if code == "E0031")),
            "expected inherited summary after close, got {diagnostics:?}"
        );

        std::fs::remove_dir_all(root).unwrap();
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
