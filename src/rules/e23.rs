use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::{Message, Violation};
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0023";
static DESCRIPTION: &str = "Instability (I), Abstractness (A), Distance from Main Sequence (D)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_instability: f64,
    pub max_abstractness: f64,
    pub max_distance: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_instability: 0.8,
            max_abstractness: 0.8,
            max_distance: 0.5,
        }
    }
}

/// Tracks namespace composition and dependencies for I/A/D computation.
#[derive(Default)]
struct PackageIndex {
    /// namespace → set of concrete class names
    concrete_classes: HashMap<String, HashSet<String>>,
    /// namespace → set of abstract class + interface names
    abstract_classes: HashMap<String, HashSet<String>>,
    /// class name → namespace
    class_to_namespace: HashMap<String, String>,
    /// class name → set of external type names it references
    class_dependencies: HashMap<String, HashSet<String>>,
    /// namespace → all classes in it
    namespace_classes: HashMap<String, HashSet<String>>,
}

pub struct Rule {
    pub settings: Settings,
    index: Mutex<PackageIndex>,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            index: Mutex::new(PackageIndex::default()),
        }
    }
}

impl RuleTrait for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from(DESCRIPTION)
    }

    fn do_validate(&self, _file: &File<'_>) -> bool {
        true
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        }
    }

    fn index_file(&self, file: &File<'_>) {
        if let Some(program) = file.ast {
            let namespace = file.namespace.clone().unwrap_or_default();
            for statement in program.statements.iter() {
                self.collect_package_data(statement, &namespace);
            }
        }
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Namespace(ns) = statement {
            let namespace = ns
                .name
                .as_ref()
                .map(|n| String::from_utf8_lossy(n.value()).into_owned())
                .unwrap_or_default();

            if namespace.is_empty() {
                return violations;
            }

            let (ca, ce, abstract_count, total_count) = self.compute_metrics(&namespace);

            // Guard against division by zero
            let total_coupling = ca + ce;

            // Instability: I = Ce / (Ca + Ce)
            if total_coupling > 0 {
                let instability = ce as f64 / total_coupling as f64;
                if instability > self.settings.max_instability {
                    let message = Message::new(
                        "E0023:instability",
                        "Namespace \"{namespace}\" has instability (I) of {instability} (threshold: {threshold}). Highly unstable namespaces depend on many others but few depend on them.",
                    )
                    .arg("namespace", namespace.to_string())
                    .arg("instability", format!("{:.2}", instability))
                    .arg("threshold", format!("{:.2}", self.settings.max_instability));
                    violations.push(self.new_violation(file, message, ns.span()));
                }

                // Abstractness: A = abstract / total
                if total_count > 0 {
                    let abstractness = abstract_count as f64 / total_count as f64;
                    if abstractness > self.settings.max_abstractness {
                        let message = Message::new(
                            "E0023:abstractness",
                            "Namespace \"{namespace}\" has abstractness (A) of {abstractness} (threshold: {threshold}). Too many abstract classes without concrete implementations.",
                        )
                        .arg("namespace", namespace.to_string())
                        .arg("abstractness", format!("{:.2}", abstractness))
                        .arg("threshold", format!("{:.2}", self.settings.max_abstractness));
                        violations.push(self.new_violation(file, message, ns.span()));
                    }

                    // Distance from Main Sequence: D = |A + I - 1|
                    let distance = (abstractness + instability - 1.0).abs();
                    if distance > self.settings.max_distance {
                        let message = Message::new(
                            "E0023:distance",
                            "Namespace \"{namespace}\" has distance from main sequence (D) of {distance} (threshold: {threshold}). Consider rebalancing abstractness and stability.",
                        )
                        .arg("namespace", namespace.to_string())
                        .arg("distance", format!("{:.2}", distance))
                        .arg("threshold", format!("{:.2}", self.settings.max_distance));
                        violations.push(self.new_violation(file, message, ns.span()));
                    }
                }
            }
        }

        violations
    }
}

impl Rule {
    fn collect_package_data(&self, statement: &Statement<'_>, namespace: &str) {
        match statement {
            Statement::Namespace(ns) => {
                let ns_name = ns
                    .name
                    .as_ref()
                    .map(|n| String::from_utf8_lossy(n.value()).into_owned())
                    .unwrap_or_default();
                for s in ns.statements().iter() {
                    self.collect_package_data(s, &ns_name);
                }
            }
            Statement::Class(class) => {
                let name = String::from_utf8_lossy(class.name.value).into_owned();
                let is_abstract = class
                    .modifiers
                    .iter()
                    .any(|m| matches!(m, Modifier::Abstract(_)));

                let mut deps = HashSet::new();
                if let Some(extends) = &class.extends {
                    for parent in extends.types.iter() {
                        deps.insert(self.normalize_type(std::str::from_utf8(parent.value()).unwrap_or_default()));
                    }
                }
                if let Some(implements) = &class.implements {
                    for iface in implements.types.iter() {
                        deps.insert(self.normalize_type(std::str::from_utf8(iface.value()).unwrap_or_default()));
                    }
                }
                for member in class.members.iter() {
                    self.collect_member_deps(member, &mut deps);
                }

                if let Ok(mut index) = self.index.lock() {
                    index
                        .class_to_namespace
                        .insert(name.clone(), namespace.to_string());
                    index
                        .namespace_classes
                        .entry(namespace.to_string())
                        .or_default()
                        .insert(name.clone());
                    index.class_dependencies.insert(name.clone(), deps);

                    if is_abstract {
                        index
                            .abstract_classes
                            .entry(namespace.to_string())
                            .or_default()
                            .insert(name);
                    } else {
                        index
                            .concrete_classes
                            .entry(namespace.to_string())
                            .or_default()
                            .insert(name);
                    }
                }
            }
            Statement::Interface(iface) => {
                let name = String::from_utf8_lossy(iface.name.value).into_owned();
                if let Ok(mut index) = self.index.lock() {
                    index
                        .class_to_namespace
                        .insert(name.clone(), namespace.to_string());
                    index
                        .namespace_classes
                        .entry(namespace.to_string())
                        .or_default()
                        .insert(name.clone());
                    // Interfaces count as abstract
                    index
                        .abstract_classes
                        .entry(namespace.to_string())
                        .or_default()
                        .insert(name);
                }
            }
            Statement::Trait(t) => {
                let name = String::from_utf8_lossy(t.name.value).into_owned();
                if let Ok(mut index) = self.index.lock() {
                    index
                        .class_to_namespace
                        .insert(name.clone(), namespace.to_string());
                    index
                        .namespace_classes
                        .entry(namespace.to_string())
                        .or_default()
                        .insert(name.clone());
                    // Traits count as concrete
                    index
                        .concrete_classes
                        .entry(namespace.to_string())
                        .or_default()
                        .insert(name);
                }
            }
            _ => {}
        }
    }

    fn collect_member_deps(&self, member: &ClassLikeMember<'_>, deps: &mut HashSet<String>) {
        match member {
            ClassLikeMember::Method(method) => {
                for param in method.parameter_list.parameters.iter() {
                    if let Some(hint) = &param.hint {
                        self.collect_hint_deps(hint, deps);
                    }
                }
                if let Some(ret) = &method.return_type_hint {
                    self.collect_hint_deps(&ret.hint, deps);
                }
            }
            ClassLikeMember::Property(property) => {
                if let Some(hint) = property.hint() {
                    self.collect_hint_deps(hint, deps);
                }
            }
            ClassLikeMember::TraitUse(trait_use) => {
                for trait_name in trait_use.trait_names.iter() {
                    deps.insert(self.normalize_type(std::str::from_utf8(trait_name.value()).unwrap_or_default()));
                }
            }
            _ => {}
        }
    }

    fn collect_hint_deps(&self, hint: &Hint<'_>, deps: &mut HashSet<String>) {
        match hint {
            Hint::Identifier(id) => {
                let name = self.normalize_type(std::str::from_utf8(id.value()).unwrap_or_default());
                if !self.is_builtin(&name) {
                    deps.insert(name);
                }
            }
            Hint::Nullable(n) => self.collect_hint_deps(n.hint, deps),
            Hint::Union(u) => {
                self.collect_hint_deps(u.left, deps);
                self.collect_hint_deps(u.right, deps);
            }
            Hint::Intersection(i) => {
                self.collect_hint_deps(i.left, deps);
                self.collect_hint_deps(i.right, deps);
            }
            Hint::Parenthesized(p) => self.collect_hint_deps(p.hint, deps),
            _ => {}
        }
    }

    /// Returns (Ca, Ce, abstract_count, total_count) for a namespace
    fn compute_metrics(&self, namespace: &str) -> (usize, usize, usize, usize) {
        let index = match self.index.lock() {
            Ok(idx) => idx,
            Err(_) => return (0, 0, 0, 0),
        };

        let our_classes = match index.namespace_classes.get(namespace) {
            Some(classes) => classes,
            None => return (0, 0, 0, 0),
        };

        let abstract_count = index
            .abstract_classes
            .get(namespace)
            .map_or(0, |s| s.len());
        let total_count = our_classes.len();

        // Ce: distinct external classes our classes depend on
        let mut ce_types: HashSet<String> = HashSet::new();
        for class_name in our_classes {
            if let Some(deps) = index.class_dependencies.get(class_name) {
                for dep in deps {
                    let dep_ns = index.class_to_namespace.get(dep);
                    if dep_ns.is_none_or(|ns| ns != namespace) {
                        ce_types.insert(dep.clone());
                    }
                }
            }
        }

        // Ca: distinct external classes that depend on our classes
        let mut ca_types: HashSet<String> = HashSet::new();
        for (class_name, deps) in &index.class_dependencies {
            let class_ns = index.class_to_namespace.get(class_name);
            if class_ns.is_some_and(|ns| ns == namespace) {
                continue;
            }
            for dep in deps {
                if our_classes.contains(dep) {
                    ca_types.insert(class_name.clone());
                }
            }
        }

        (ca_types.len(), ce_types.len(), abstract_count, total_count)
    }

    fn normalize_type(&self, name: &str) -> String {
        let normalized = name.trim_start_matches('\\');
        normalized
            .rsplit('\\')
            .next()
            .unwrap_or(normalized)
            .to_string()
    }

    fn is_builtin(&self, name: &str) -> bool {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "array"
                | "bool"
                | "boolean"
                | "callable"
                | "false"
                | "float"
                | "int"
                | "integer"
                | "iterable"
                | "mixed"
                | "never"
                | "null"
                | "object"
                | "resource"
                | "string"
                | "true"
                | "void"
                | "self"
                | "static"
                | "parent"
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn balanced_namespace() {
        let violations = analyze_file_for_rule("e23/balanced.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn empty_namespace_no_crash() {
        let violations = analyze_file_for_rule("e23/empty_namespace.php", CODE);
        assert_eq!(violations.len(), 0);
    }
}
