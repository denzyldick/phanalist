use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

pub static CODE: &str = "E0021";
static DESCRIPTION: &str = "Number of Children (NOC)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_children: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_children: 15 }
    }
}

/// Global index: parent class name → set of child class names.
#[derive(Default)]
struct ChildrenIndex {
    children: HashMap<String, HashSet<String>>,
}

pub struct Rule {
    pub settings: Settings,
    index: Mutex<ChildrenIndex>,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            index: Mutex::new(ChildrenIndex::default()),
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
                self.collect_children(statement);
            }
        }
    }

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            let class_name = class.name.value.to_string();
            let child_count = self.get_child_count(&class_name);

            if child_count > self.settings.max_children {
                let suggestion = format!(
                    "Class \"{}\" has {} direct subclasses (threshold: {}). High NOC increases the impact of changes.",
                    class_name, child_count, self.settings.max_children
                );
                violations.push(self.new_violation(file, suggestion, class.span()));
            }
        }

        violations
    }
}

impl Rule {
    fn collect_children(&self, statement: &Statement<'_>) {
        match statement {
            Statement::Namespace(ns) => {
                for s in ns.statements().iter() {
                    self.collect_children(s);
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
                            index
                                .children
                                .entry(parent_name)
                                .or_default()
                                .insert(child);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn get_child_count(&self, class_name: &str) -> usize {
        match self.index.lock() {
            Ok(index) => index
                .children
                .get(class_name)
                .map_or(0, |children| children.len()),
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn many_children() {
        let violations = analyze_file_for_rule("e21/many_children.php", CODE);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].suggestion.contains("direct subclasses"));
    }

    #[test]
    fn few_children() {
        let violations = analyze_file_for_rule("e21/few_children.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn no_children() {
        let violations = analyze_file_for_rule("e21/no_children.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn interface_not_counted() {
        let violations = analyze_file_for_rule("e21/interface_not_counted.php", CODE);
        assert_eq!(violations.len(), 0);
    }
}
