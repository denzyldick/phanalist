use std::collections::HashMap;
use std::sync::Mutex;

use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0020";
static DESCRIPTION: &str = "Depth of Inheritance Tree (DIT)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_depth: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_depth: 4 }
    }
}

/// Global index: child class name → parent class name.
#[derive(Default)]
struct InheritanceIndex {
    extends: HashMap<String, String>,
}

pub struct Rule {
    pub settings: Settings,
    index: Mutex<InheritanceIndex>,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            index: Mutex::new(InheritanceIndex::default()),
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
            for statement in program.statements.iter() {
                self.collect_extends(statement);
            }
        }
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            let class_name = class.name.value.to_string();
            let depth = self.compute_depth(&class_name);

            if depth > self.settings.max_depth {
                let suggestion = format!(
                    "Class \"{}\" has an inheritance depth of {} (threshold: {}). Deep hierarchies increase complexity.",
                    class_name, depth, self.settings.max_depth
                );
                violations.push(self.new_violation(file, suggestion, class.span()));
            }
        }

        violations
    }
}

impl Rule {
    fn collect_extends(&self, statement: &Statement<'_>) {
        match statement {
            Statement::Namespace(ns) => {
                for s in ns.statements().iter() {
                    self.collect_extends(s);
                }
            }
            Statement::Class(class) => {
                if let Some(extends) = &class.extends {
                    for parent in extends.types.iter() {
                        let child = class.name.value.to_string();
                        let parent_name = parent
                            .value()
                            .trim_start_matches('\\')
                            .rsplit('\\')
                            .next()
                            .unwrap_or(parent.value())
                            .to_string();
                        if let Ok(mut index) = self.index.lock() {
                            index.extends.insert(child, parent_name);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn compute_depth(&self, class_name: &str) -> usize {
        let index = match self.index.lock() {
            Ok(idx) => idx,
            Err(_) => return 0,
        };

        let mut depth = 0;
        let mut current = class_name.to_string();
        // Guard against circular inheritance — cap at 100 iterations
        let max_iterations = 100;
        let mut iterations = 0;

        while let Some(parent) = index.extends.get(&current) {
            depth += 1;
            current = parent.clone();
            iterations += 1;
            if iterations >= max_iterations {
                break;
            }
        }

        depth
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn deep_inheritance() {
        let violations = analyze_file_for_rule("e20/deep_inheritance.php", CODE);
        // E has depth 4, F has depth 5 — both exceed default threshold of 4
        assert!(violations.len() >= 1);
        assert!(violations
            .iter()
            .any(|v| v.suggestion.contains("inheritance depth")));
    }

    #[test]
    fn shallow_inheritance() {
        let violations = analyze_file_for_rule("e20/shallow_inheritance.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn no_parent() {
        let violations = analyze_file_for_rule("e20/no_parent.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn interface_not_counted() {
        let violations = analyze_file_for_rule("e20/interface_not_counted.php", CODE);
        assert_eq!(violations.len(), 0);
    }
}
