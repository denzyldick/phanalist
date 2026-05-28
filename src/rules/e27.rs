use mago_span::HasSpan;
use mago_syntax::ast::{ClassLikeMember, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::rules::Rule as RuleTrait;

pub(crate) static CODE: &str = "E0027";
static DESCRIPTION: &str = "God Class (Brain Class)";

#[derive(Deserialize, Serialize)]
pub struct Settings {
    pub max_methods: usize,
    pub max_fields: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_methods: 15,
            max_fields: 10,
        }
    }
}

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
}

impl Rule {
    fn check_members(
        &self,
        file: &File<'_>,
        name: &str,
        members: &mago_syntax::ast::Sequence<'_, ClassLikeMember<'_>>,
        span: mago_span::Span,
        violations: &mut Vec<Violation>,
    ) {
        let mut method_count = 0;
        let mut field_count = 0;

        for member in members.iter() {
            match member {
                ClassLikeMember::Method(_) => method_count += 1,
                ClassLikeMember::Property(_) => field_count += 1,
                _ => {}
            }
        }

        if method_count > self.settings.max_methods && field_count > self.settings.max_fields {
            let suggestion = format!(
                "\"{}\" is a potential God Class: {} methods and {} fields (max: {} methods, {} fields). Consider splitting it into multiple types with single responsibilities.",
                name, method_count, field_count,
                self.settings.max_methods, self.settings.max_fields
            );
            violations.push(self.new_violation(file, suggestion, span));
        }
    }
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

        match statement {
            Statement::Class(class) => {
                self.check_members(file, class.name.value, &class.members, class.span(), &mut violations);
            }
            Statement::Trait(t) => {
                self.check_members(file, t.name.value, &t.members, t.span(), &mut violations);
            }
            Statement::Enum(e) => {
                self.check_members(file, e.name.value, &e.members, e.span(), &mut violations);
            }
            _ => {}
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn god_class() {
        let violations = analyze_file_for_rule("e27/god_class.php", CODE);
        assert!(violations.len().gt(&0));
        assert!(violations[0].suggestion.contains("GodClass"));
    }

    #[test]
    fn cohesive_class() {
        let violations = analyze_file_for_rule("e27/cohesive_class.php", CODE);
        assert!(violations.len().eq(&0));
    }
}
