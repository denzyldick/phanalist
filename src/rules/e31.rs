use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::verification;

pub(crate) static CODE: &str = "E0031";
static DESCRIPTION: &str = "Formal Verification";

pub use crate::verification::Settings;

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
        false
    }

    fn set_config(&mut self, json: &Value) {
        match serde_json::from_value(json.to_owned()) {
            Ok(settings) => self.settings = settings,
            Err(e) => self.output_error(e.into()),
        }
    }

    fn validate(&self, _file: &File<'_>, _statement: &mago_syntax::cst::Statement<'_>) -> Vec<Violation> {
        Vec::new()
    }

    fn validate_file(&self, file: &File<'_>) -> Option<Vec<Violation>> {
        let findings = verification::analyze_file(file, &self.settings);
        Some(
            findings
                .into_iter()
                .map(|finding| self.new_violation(file, finding.message, finding.span))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    #[test]
    fn reports_literal_division_by_zero() {
        let violations = analyze_file_for_rule("e31/division_by_zero_literal.php", CODE);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.render().contains("Division by zero"));
    }

    #[test]
    fn proves_nonzero_literal_division_safe() {
        let violations = analyze_file_for_rule("e31/division_by_nonzero.php", CODE);

        assert!(violations.is_empty());
    }

    #[test]
    fn reports_unconstrained_divisor() {
        let violations = analyze_file_for_rule("e31/division_by_parameter.php", CODE);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.render().contains("Division by zero"));
    }

    #[test]
    fn proves_guarded_division_safe() {
        let violations = analyze_file_for_rule("e31/division_guarded.php", CODE);

        assert!(violations.is_empty());
    }

    #[test]
    fn reports_unconstrained_modulo() {
        let violations = analyze_file_for_rule("e31/modulo_by_parameter.php", CODE);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.render().contains("Modulo by zero"));
    }

    #[test]
    fn reports_unconstrained_intdiv() {
        let violations = analyze_file_for_rule("e31/intdiv_by_parameter.php", CODE);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.render().contains("intdiv() by zero"));
    }

    #[test]
    fn reports_intdiv_overflow() {
        let violations = analyze_file_for_rule("e31/intdiv_overflow.php", CODE);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.render().contains("overflows"));
    }
}
