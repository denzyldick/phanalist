use mago_span::HasSpan;
use mago_syntax::ast::Statement;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;

pub(crate) static CODE: &str = "E0025";
static DESCRIPTION: &str = "Lines of Code (LOC) per File";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_loc: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { max_loc: 500 }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl crate::rules::Rule for Rule {
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

        let name = match statement {
            Statement::Class(class) => Some((class.name.value, class.span())),
            Statement::Trait(t) => Some((t.name.value, t.span())),
            Statement::Enum(e) => Some((e.name.value, e.span())),
            Statement::Interface(i) => Some((i.name.value, i.span())),
            _ => None,
        };

        if let Some((name, span)) = name {
            let loc = file.lines.len();
            if loc > self.settings.max_loc {
                let suggestion = format!(
                    "File containing \"{}\" has {} lines of code (max: {}). Consider splitting it into smaller files.",
                    name, loc, self.settings.max_loc
                );
                violations.push(self.new_violation(file, suggestion, span));
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
    fn long_file() {
        let violations = analyze_file_for_rule("e25/long_file.php", CODE);
        assert!(violations.len().gt(&0));
    }

    #[test]
    fn short_file() {
        let violations = analyze_file_for_rule("e25/short_file.php", CODE);
        assert!(violations.len().eq(&0));
    }
}
