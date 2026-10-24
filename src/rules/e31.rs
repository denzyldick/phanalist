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

    fn validate(
        &self,
        _file: &File<'_>,
        _statement: &mago_syntax::cst::Statement<'_>,
    ) -> Vec<Violation> {
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

#[cfg(test)]
mod verification_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn proves_array_access_in_bounds() {
        assert!(messages("e31/array_in_bounds.php").is_empty());
    }

    #[test]
    fn reports_array_access_out_of_bounds() {
        let messages = messages("e31/array_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Array key"));
    }

    #[test]
    fn reports_missing_string_array_key() {
        let messages = messages("e31/array_missing_string_key.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Array key"));
    }

    #[test]
    fn reports_symbolic_array_access_out_of_bounds() {
        let messages = messages("e31/array_symbolic_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Array key"));
    }

    #[test]
    fn proves_string_offset_in_bounds() {
        assert!(messages("e31/string_offset_in_bounds.php").is_empty());
    }

    #[test]
    fn proves_negative_string_offset_in_bounds() {
        assert!(messages("e31/string_offset_negative.php").is_empty());
    }

    #[test]
    fn reports_string_offset_out_of_bounds() {
        let messages = messages("e31/string_offset_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("String offset"));
    }

    #[test]
    fn reports_method_call_on_null() {
        let messages = messages("e31/null_method_call.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Method call on null"));
    }

    #[test]
    fn allows_null_safe_method_call() {
        assert!(messages("e31/nullsafe_method_call.php").is_empty());
    }

    #[test]
    fn reports_property_access_on_null() {
        let messages = messages("e31/null_property_access.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Property access on null"));
    }

    #[test]
    fn assertion_narrows_the_path() {
        assert!(messages("e31/assertion_guard.php").is_empty());
    }

    #[test]
    fn reports_assertion_that_can_never_hold() {
        let messages = messages("e31/assertion_always_false.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("assertion can never hold"));
    }

    #[test]
    fn reports_return_contract_mismatch() {
        let messages = messages("e31/return_contract_bad_string.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("does not satisfy"));
    }

    #[test]
    fn reports_strict_return_contract_mismatch() {
        let messages = messages("e31/return_contract_strict.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("does not satisfy"));
    }

    #[test]
    fn allows_weak_return_coercion() {
        assert!(messages("e31/return_contract_weak.php").is_empty());
    }

    #[test]
    fn reports_missing_return_value() {
        let messages = messages("e31/return_contract_missing.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("return without a value"));
    }

    #[test]
    fn reports_unreachable_statement() {
        let messages = messages("e31/unreachable_after_return.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }
}

#[cfg(test)]
mod context_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn does_not_flag_isset_on_null_property() {
        assert!(messages("e31/isset_null_property.php").is_empty());
    }

    #[test]
    fn does_not_flag_isset_on_missing_array_key() {
        assert!(messages("e31/isset_missing_array_key.php").is_empty());
    }

    #[test]
    fn does_not_flag_null_coalesce_on_null_property() {
        assert!(messages("e31/coalesce_null_property.php").is_empty());
    }
}

#[cfg(test)]
mod loop_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn finds_division_by_zero_on_first_loop_iteration() {
        let messages = messages("e31/while_division_first_iteration.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn unrolls_guarded_while_loop() {
        assert!(messages("e31/while_division_guarded.php").is_empty());
    }

    #[test]
    fn unrolls_guarded_for_loop() {
        assert!(messages("e31/for_division_guarded.php").is_empty());
    }

    #[test]
    fn break_makes_the_code_after_the_loop_reachable() {
        assert!(messages("e31/loop_break_reachable.php").is_empty());
    }

    #[test]
    fn return_in_an_infinite_loop_makes_following_code_unreachable() {
        let messages = messages("e31/loop_return_unreachable.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }
}
