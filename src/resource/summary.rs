use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mago_syntax::cst::{ClassLikeMember, MethodBody, Program, Statement};

use crate::file::File;

use super::model::{CostExpr, HeapEffect, SizeExpr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceSummary {
    pub params: Vec<String>,
    pub cost: CostExpr,
    pub heap: HeapEffect,
    pub return_count: SizeExpr,
    pub return_bytes: SizeExpr,
    pub return_tainted_from: Vec<usize>,
    pub sensitive_params: Vec<String>,
    pub recursive: bool,
}

#[derive(Default)]
pub struct ResourceSummaryIndex {
    files: HashMap<PathBuf, Vec<(String, ResourceSummary)>>,
    snapshot_cache: Mutex<Option<Arc<HashMap<String, ResourceSummary>>>>,
}

impl ResourceSummaryIndex {
    pub fn clear(&mut self) {
        self.files.clear();
        self.invalidate();
    }

    pub fn remove_file(&mut self, path: &Path) {
        self.files.remove(path);
        self.invalidate();
    }

    pub fn replace_file(&mut self, file: &File<'_>) {
        self.files.remove(&file.path);
        self.invalidate();

        let Some(program) = file.ast else {
            return;
        };

        let mut summaries = Vec::new();
        collect_file_summaries(file, program, &mut summaries);
        self.files.insert(file.path.clone(), summaries);
    }

    pub fn snapshot(&self) -> Arc<HashMap<String, ResourceSummary>> {
        if let Some(snapshot) = self
            .snapshot_cache
            .lock()
            .ok()
            .and_then(|cache| cache.clone())
        {
            return snapshot;
        }

        let mut counts: HashMap<String, usize> = HashMap::new();
        for (name, _) in self.files.values().flat_map(|entries| entries.iter()) {
            *counts.entry(name.clone()).or_default() += 1;
        }

        let mut summaries = HashMap::new();
        for (name, summary) in self.files.values().flat_map(|entries| entries.iter()) {
            if counts.get(name) == Some(&1) {
                summaries.insert(name.clone(), summary.clone());
            }
        }

        let snapshot = Arc::new(summaries);
        if let Ok(mut cache) = self.snapshot_cache.lock() {
            *cache = Some(snapshot.clone());
        }
        snapshot
    }

    fn invalidate(&self) {
        if let Ok(mut cache) = self.snapshot_cache.lock() {
            *cache = None;
        }
    }
}

fn collect_file_summaries(
    file: &File<'_>,
    program: &Program<'_>,
    summaries: &mut Vec<(String, ResourceSummary)>,
) {
    collect_statements(file, program.statements.as_slice(), summaries);
}

fn collect_statements(
    file: &File<'_>,
    statements: &[Statement<'_>],
    summaries: &mut Vec<(String, ResourceSummary)>,
) {
    for statement in statements {
        match statement {
            Statement::Namespace(namespace) => {
                let nested = namespace.statements().as_slice();
                collect_statements(file, nested, summaries);
            }
            Statement::Function(function) => {
                let name = String::from_utf8_lossy(function.name.value).to_string();
                let params = function
                    .parameter_list
                    .parameters
                    .iter()
                    .map(|parameter| String::from_utf8_lossy(parameter.variable.name).to_string())
                    .collect::<Vec<_>>();
                if let Some(summary) = super::analyzer::summarize_function(
                    file,
                    &name,
                    &params,
                    function.body.statements.as_slice(),
                ) {
                    summaries.push((name, summary));
                }
            }
            Statement::Class(class) => {
                let name = String::from_utf8_lossy(class.name.value).to_string();
                collect_members(file, &name, class.members.iter(), summaries);
            }
            Statement::Interface(interface) => {
                let name = String::from_utf8_lossy(interface.name.value).to_string();
                collect_members(file, &name, interface.members.iter(), summaries);
            }
            Statement::Trait(r#trait) => {
                let name = String::from_utf8_lossy(r#trait.name.value).to_string();
                collect_members(file, &name, r#trait.members.iter(), summaries);
            }
            Statement::Enum(r#enum) => {
                let name = String::from_utf8_lossy(r#enum.name.value).to_string();
                collect_members(file, &name, r#enum.members.iter(), summaries);
            }
            Statement::Block(block) => {
                collect_statements(file, block.statements.as_slice(), summaries);
            }
            Statement::Declare(declare) => {
                if let mago_syntax::cst::DeclareBody::Statement(statement) = &declare.body {
                    collect_statements(file, std::slice::from_ref(statement), summaries);
                }
            }
            _ => {}
        }
    }
}

fn collect_members<'arena>(
    file: &File<'_>,
    class_name: &str,
    members: impl Iterator<Item = &'arena ClassLikeMember<'arena>>,
    summaries: &mut Vec<(String, ResourceSummary)>,
) {
    for member in members {
        let ClassLikeMember::Method(method) = member else {
            continue;
        };
        let MethodBody::Concrete(body) = &method.body else {
            continue;
        };
        let method_name = String::from_utf8_lossy(method.name.value).to_string();
        let params = method
            .parameter_list
            .parameters
            .iter()
            .map(|parameter| String::from_utf8_lossy(parameter.variable.name).to_string())
            .collect::<Vec<_>>();
        let Some(summary) = super::analyzer::summarize_function(
            file,
            &method_name,
            &params,
            body.statements.as_slice(),
        ) else {
            continue;
        };

        summaries.push((format!("{class_name}::{method_name}"), summary.clone()));
        summaries.push((method_name, summary));
    }
}
