use std::{collections::HashMap, path::PathBuf};

use bumpalo::Bump;
use mago_database::file::FileId;
use mago_span::Span;
use mago_syntax::ast::{Program, Statement};
use serde::{Deserialize, Serialize};

/// A PHP source file paired with its parsed AST.
///
/// The AST is allocated inside an external `bumpalo::Bump` arena; the same arena
/// is reused across every file in a scan and freed in one go when the scan ends,
/// instead of doing per-file heap allocations.
#[derive(Debug, Clone)]
pub struct File<'arena> {
    pub path: PathBuf,
    pub lines: Vec<String>,
    pub line_starts: Vec<u32>,
    pub namespace: Option<String>,
    pub class_name: Option<String>,
    pub reference_counter: RC,
    pub ast: Option<&'arena Program<'arena>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RC {
    pub methods: HashMap<String, Method>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Method {
    pub name: String,
    pub span: Span,
    pub counter: isize,
}

impl Method {
    pub fn increase_counter(&mut self) {
        self.counter += 1;
    }
}

impl RC {
    pub fn new() -> Self {
        Self {
            methods: HashMap::new(),
        }
    }

    // TODO: Implement reference counting with mago_syntax AST.
    pub fn build_reference_counter(&mut self, _program: &Program<'_>) -> Option<RC> {
        None
    }
}

impl<'arena> File<'arena> {
    pub fn new(arena: &'arena Bump, path: PathBuf, content: String) -> Self {
        let file_id = FileId::new(&path.to_string_lossy());
        let program = mago_syntax::parser::parse_file_content(arena, file_id, &content);

        let line_starts = compute_line_starts(&content);
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        let (namespace, class_name) = extract_namespace_and_class(program);

        Self {
            path,
            lines,
            line_starts,
            namespace,
            class_name,
            reference_counter: RC::new(),
            ast: Some(program),
        }
    }

    pub fn get_class(&self) -> Option<Vec<Statement<'arena>>> {
        // TODO: Implement with mago_syntax AST.
        None
    }

    pub fn get_fully_qualified_name(&self) -> Option<String> {
        match &self.namespace {
            Some(n) => self.class_name.as_ref().map(|s| format!("{}\\{}", n, s)),
            None => self.class_name.clone(),
        }
    }

    /// 1-based line number for a byte offset within this file's content.
    pub fn line_number(&self, offset: u32) -> usize {
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i + 1,
            Err(i) => i,
        }
    }

    /// 0-based column (byte) for a byte offset within this file's content.
    pub fn column_number(&self, offset: u32) -> usize {
        let line = self.line_number(offset);
        let line_start = if line == 0 {
            0
        } else {
            self.line_starts[line - 1]
        };
        (offset - line_start) as usize
    }
}

/// Compute the byte offset of the start of each line in `content`.
///
/// Handles Unix (`\n`), Windows (`\r\n`), and old-Mac (`\r`) line endings. The style
/// is picked from the first line ending encountered; real files use a single
/// convention throughout, so we don't try to handle mixed endings.
///
/// Copied from [`mago_database::file::line_starts`](https://github.com/carthage-software/mago/blob/2f248e8a4bac057ead9eee62dbd7ba91e4fffc6f/crates/database/src/file.rs#L248-L282)
/// with modifications.
fn compute_line_starts(content: &str) -> Vec<u32> {
    let bytes = content.as_bytes();
    let mut starts = vec![0u32];

    let mut use_cr = false;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            break;
        }
        if b == b'\r' {
            // Bare `\r` (old Mac) if not immediately followed by `\n`; otherwise
            // Windows `\r\n`, which the `\n` scan below already handles.
            use_cr = bytes.get(i + 1) != Some(&b'\n');
            break;
        }
    }

    let marker = if use_cr { b'\r' } else { b'\n' };
    for (i, &b) in bytes.iter().enumerate() {
        if b == marker {
            starts.push((i + 1) as u32);
        }
    }

    starts
}

fn extract_namespace_and_class<'arena>(
    program: &Program<'arena>,
) -> (Option<String>, Option<String>) {
    let mut namespace = None;
    let mut class_name = None;

    for statement in program.statements.iter() {
        match statement {
            Statement::Namespace(ns) => {
                if let Some(name) = ns.name.as_ref() {
                    namespace = Some(name.value().to_string());
                }
                for s in ns.statements().iter() {
                    if let Statement::Class(class) = s {
                        class_name = Some(class.name.value.to_string());
                        break;
                    }
                }
            }
            Statement::Class(class) => {
                class_name = Some(class.name.value.to_string());
            }
            _ => {}
        }

        if class_name.is_some() {
            break;
        }
    }

    (namespace, class_name)
}
