use mago_span::HasSpan;
use mago_syntax::ast::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::e9::calculate_complexity;
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0018";
static DESCRIPTION: &str = "Weighted Methods per Class (WMC)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_wmc: i64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_wmc: 50 }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
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

    fn validate(&self, file: &File<'_>, statement: &Statement<'_>) -> Vec<Violation> {
        let mut violations = Vec::new();

        if let Statement::Class(class) = statement {
            let mut wmc: i64 = 0;

            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member {
                    if let MethodBody::Concrete(block) = &method.body {
                        // Base complexity is 1 for the method itself, plus internal branches
                        wmc += 1 + calculate_complexity(&block.statements);
                    }
                    // Abstract methods have no body → CC = 0, do not contribute
                }
            }

            if wmc > self.settings.max_wmc {
                let suggestion = format!(
                    "Class \"{}\" has a Weighted Methods per Class (WMC) of {} (threshold: {}). Consider splitting responsibilities.",
                    String::from_utf8_lossy(class.name.value), wmc, self.settings.max_wmc
                );
                violations.push(self.new_violation(file, suggestion, class.span()));
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn high_wmc() {
        let violations = analyze_file_for_rule("e18/high_wmc.php", CODE);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].suggestion.contains("Weighted Methods per Class (WMC)"));
    }

    #[test]
    fn low_wmc() {
        let violations = analyze_file_for_rule("e18/low_wmc.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn empty_class() {
        let violations = analyze_file_for_rule("e18/empty_class.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn single_simple_method() {
        let violations = analyze_file_for_rule("e18/single_simple_method.php", CODE);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn interface_ignored() {
        let violations = analyze_file_for_rule("e18/interface_ignored.php", CODE);
        assert_eq!(violations.len(), 0);
    }
}
