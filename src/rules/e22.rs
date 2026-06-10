use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0022";
static DESCRIPTION: &str = "Afferent and Efferent Coupling (Ca/Ce)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_ca: usize,
    pub max_ce: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_ca: 20,
            max_ce: 20,
        }
    }
}

/// Tracks which namespace each class belongs to, and what external types each class references.
#[derive(Default)]
struct NamespaceIndex {
    /// class name → namespace it belongs to
    class_to_namespace: HashMap<String, String>,
    /// namespace → set of classes defined in it
    namespace_classes: HashMap<String, HashSet<String>>,
    /// class name → set of external type names it references
    class_dependencies: HashMap<String, HashSet<String>>,
}

pub struct Rule {
    pub settings: Settings,
    index: Mutex<NamespaceIndex>,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            index: Mutex::new(NamespaceIndex::default()),
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
                self.collect_namespace_data(statement, &namespace);
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

            let (ca, ce) = self.compute_coupling(&namespace);

            if ca > self.settings.max_ca {
                let suggestion = format!(
                    "Namespace \"{}\" has afferent coupling (Ca) of {} (threshold: {}). Many external classes depend on this namespace.",
                    namespace, ca, self.settings.max_ca
                );
                violations.push(self.new_violation(file, suggestion, ns.span()));
            }

            if ce > self.settings.max_ce {
                let suggestion = format!(
                    "Namespace \"{}\" has efferent coupling (Ce) of {} (threshold: {}). This namespace depends on too many external classes.",
                    namespace, ce, self.settings.max_ce
                );
                violations.push(self.new_violation(file, suggestion, ns.span()));
            }
        }

        violations
    }
}

impl Rule {
    fn collect_namespace_data(&self, statement: &Statement<'_>, namespace: &str) {
        match statement {
            Statement::Namespace(ns) => {
                let ns_name = ns
                    .name
                    .as_ref()
                    .map(|n| String::from_utf8_lossy(n.value()).into_owned())
                    .unwrap_or_default();
                for s in ns.statements().iter() {
                    self.collect_namespace_data(s, &ns_name);
                }
            }
            Statement::Class(class) => {
                let class_name = String::from_utf8_lossy(class.name.value).into_owned();
                let mut deps = HashSet::new();

                // extends
                if let Some(extends) = &class.extends {
                    for parent in extends.types.iter() {
                        deps.insert(self.normalize_type(std::str::from_utf8(parent.value()).unwrap_or_default()));
                    }
                }

                // implements
                if let Some(implements) = &class.implements {
                    for iface in implements.types.iter() {
                        deps.insert(self.normalize_type(std::str::from_utf8(iface.value()).unwrap_or_default()));
                    }
                }

                // member type hints
                for member in class.members.iter() {
                    self.collect_member_deps(member, &mut deps);
                }

                if let Ok(mut index) = self.index.lock() {
                    index
                        .class_to_namespace
                        .insert(class_name.clone(), namespace.to_string());
                    index
                        .namespace_classes
                        .entry(namespace.to_string())
                        .or_default()
                        .insert(class_name.clone());
                    index.class_dependencies.insert(class_name, deps);
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
                        .insert(name);
                }
            }
            _ => {}
        }
    }

    fn collect_member_deps(&self, member: &ClassLikeMember<'_>, deps: &mut HashSet<String>) {
        match member {
            ClassLikeMember::Method(method) => {
                // Parameter type hints
                for param in method.parameter_list.parameters.iter() {
                    if let Some(hint) = &param.hint {
                        self.collect_hint_deps(hint, deps);
                    }
                }
                // Return type hint
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

    fn compute_coupling(&self, namespace: &str) -> (usize, usize) {
        let index = match self.index.lock() {
            Ok(idx) => idx,
            Err(_) => return (0, 0),
        };

        let our_classes = match index.namespace_classes.get(namespace) {
            Some(classes) => classes,
            None => return (0, 0),
        };

        // Ce: count distinct external classes that OUR classes depend on
        let mut ce_types: HashSet<String> = HashSet::new();
        for class_name in our_classes {
            if let Some(deps) = index.class_dependencies.get(class_name) {
                for dep in deps {
                    // Only count if the dependency is NOT in our namespace
                    let dep_ns = index.class_to_namespace.get(dep);
                    if dep_ns.is_none_or(|ns| ns != namespace) {
                        ce_types.insert(dep.clone());
                    }
                }
            }
        }

        // Ca: count distinct external classes that depend on OUR classes
        let mut ca_types: HashSet<String> = HashSet::new();
        for (class_name, deps) in &index.class_dependencies {
            // Skip classes in our own namespace
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

        (ca_types.len(), ce_types.len())
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
    fn self_contained_namespace() {
        let violations = analyze_file_for_rule("e22/self_contained.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn no_namespace_no_crash() {
        let violations = analyze_file_for_rule("e22/no_namespace.php", CODE);
        assert_eq!(violations.len(), 0);
    }
}
