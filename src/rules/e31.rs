use serde_json::Value;

use crate::file::File;
use crate::results::Violation;
use crate::verification::{self, SummaryIndex};

pub(crate) static CODE: &str = "E0031";
static DESCRIPTION: &str = "Formal Verification";

pub use crate::verification::Settings;

#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
    pub summaries: std::sync::Mutex<SummaryIndex>,
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

    fn begin_index(&self) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.clear();
        }
    }

    fn index_file(&self, file: &File<'_>) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.replace_file(file);
        }
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
        let summaries = self
            .summaries
            .lock()
            .map(|summaries| summaries.snapshot())
            .unwrap_or_default();
        let findings = verification::analyze_file_with_summaries(file, &self.settings, &summaries);
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

#[cfg(test)]
mod builtin_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn proves_a_strlen_derived_string_offset() {
        assert!(messages("e31/strlen_offset_in_bounds.php").is_empty());
    }

    #[test]
    fn reports_a_strlen_derived_string_offset_out_of_bounds() {
        let messages = messages("e31/strlen_offset_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("String offset"));
    }

    #[test]
    fn proves_a_count_derived_array_index() {
        assert!(messages("e31/count_array_in_bounds.php").is_empty());
    }

    #[test]
    fn reports_a_count_derived_array_index_out_of_bounds() {
        let messages = messages("e31/count_array_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Array key"));
    }
}

#[cfg(test)]
mod foreach_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn reports_division_by_a_zero_array_element() {
        let messages = messages("e31/foreach_array_zero.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn proves_division_by_every_array_element() {
        assert!(messages("e31/foreach_array_guarded.php").is_empty());
    }

    #[test]
    fn unrolls_key_value_array_iteration() {
        assert!(messages("e31/foreach_key_value.php").is_empty());
    }

    #[test]
    fn invalidates_a_tracked_array_after_element_assignment() {
        assert!(messages("e31/foreach_array_reassigned.php").is_empty());
    }
}

#[cfg(test)]
mod phpdoc_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn proves_division_by_a_positive_int_parameter() {
        assert!(messages("e31/phpdoc_param_positive.php").is_empty());
    }

    #[test]
    fn reports_division_by_an_unconstrained_documented_int() {
        let messages = messages("e31/phpdoc_param_unconstrained.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn reports_a_documented_return_type_mismatch() {
        let messages = messages("e31/phpdoc_return_bad.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("does not satisfy"));
    }

    #[test]
    fn accepts_a_matching_documented_return_type() {
        assert!(messages("e31/phpdoc_return_good.php").is_empty());
    }

    #[test]
    fn reports_a_documented_positive_int_violation() {
        let messages = messages("e31/phpdoc_return_negative.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("does not satisfy"));
    }

    #[test]
    fn accepts_a_nullable_documented_return_type() {
        assert!(messages("e31/phpdoc_nullable_return.php").is_empty());
    }

    #[test]
    fn reports_division_by_an_unconstrained_float_parameter() {
        let messages = messages("e31/phpdoc_param_float.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }
}

#[cfg(test)]
mod switch_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn proves_a_switch_case_guard() {
        assert!(messages("e31/switch_division_guarded.php").is_empty());
    }

    #[test]
    fn reports_division_in_a_zero_switch_case() {
        let messages = messages("e31/switch_division_zero.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn follows_switch_case_fallthrough() {
        let messages = messages("e31/switch_fallthrough_zero.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn proves_a_match_arm_guard() {
        assert!(messages("e31/match_division_guarded.php").is_empty());
    }

    #[test]
    fn reports_division_in_a_zero_match_arm() {
        let messages = messages("e31/match_division_zero.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn break_inside_switch_does_not_exit_the_surrounding_loop() {
        assert!(messages("e31/switch_break_inside_loop.php").is_empty());
    }
}

#[cfg(test)]
mod summary_tests {
    use std::fs;
    use std::path::PathBuf;

    use mago_allocator::prelude::LocalArena;

    use crate::file::File;
    use crate::rules::Rule as _;

    use super::*;

    fn fixture(path: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("rules")
            .join("examples")
            .join(path)
    }

    #[test]
    fn uses_a_constant_return_summary_in_the_same_file() {
        let path = fixture("e31/summary_same_file.php");
        let content = fs::read_to_string(&path).unwrap();
        let arena = LocalArena::new();
        let file = File::new(&arena, path, content);
        let rule = Rule::default();

        rule.begin_index();
        rule.index_file(&file);
        rule.finalize_index();

        let violations = rule.validate_file(&file).unwrap();

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.render().contains("Division by zero"));
    }

    #[test]
    fn uses_static_and_instance_method_summaries_in_the_same_file() {
        let path = fixture("e31/method_summary_same_file.php");
        let content = fs::read_to_string(&path).unwrap();
        let arena = LocalArena::new();
        let file = File::new(&arena, path, content);
        let rule = Rule::default();

        rule.begin_index();
        rule.index_file(&file);
        rule.finalize_index();

        let violations = rule.validate_file(&file).unwrap();

        assert_eq!(violations.len(), 2);
        assert!(
            violations
                .iter()
                .all(|violation| violation.message.render().contains("Division by zero"))
        );
    }

    #[test]
    fn propagates_identity_arguments_through_functions_and_methods() {
        let path = fixture("e31/summary_identity.php");
        let content = fs::read_to_string(&path).unwrap();
        let arena = LocalArena::new();
        let file = File::new(&arena, path, content);
        let rule = Rule::default();

        rule.begin_index();
        rule.index_file(&file);
        rule.finalize_index();

        let violations = rule.validate_file(&file).unwrap();

        assert_eq!(violations.len(), 3);
        assert!(
            violations
                .iter()
                .all(|violation| violation.message.render().contains("Division by zero"))
        );
    }

    #[test]
    fn uses_cross_file_static_method_summaries() {
        let definition_path = fixture("e31/method_summary_definition.php");
        let use_path = fixture("e31/method_summary_use.php");
        let definition = fs::read_to_string(&definition_path).unwrap();
        let use_content = fs::read_to_string(&use_path).unwrap();

        let arena = LocalArena::new();
        let use_file = File::new(&arena, use_path, use_content);
        let rule = Rule::default();

        rule.begin_index();
        {
            let arena = LocalArena::new();
            let file = File::new(&arena, definition_path, definition);
            rule.index_file(&file);
        }
        rule.finalize_index();

        let violations = rule.validate_file(&use_file).unwrap();

        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.render().contains("Division by zero"));
    }

    #[test]
    fn uses_cross_file_summaries_and_replaces_them_on_reindex() {
        let definition_path = fixture("e31/summary_definition.php");
        let use_path = fixture("e31/summary_use.php");
        let definition = fs::read_to_string(&definition_path).unwrap();
        let use_content = fs::read_to_string(&use_path).unwrap();

        let arena = LocalArena::new();
        let use_file = File::new(&arena, use_path, use_content);
        let rule = Rule::default();

        rule.begin_index();
        {
            let arena = LocalArena::new();
            let file = File::new(&arena, definition_path.clone(), definition);
            rule.index_file(&file);
        }
        rule.finalize_index();
        assert_eq!(rule.validate_file(&use_file).unwrap().len(), 1);

        {
            let arena = LocalArena::new();
            let file = File::new(
                &arena,
                definition_path,
                "<?php\n\nfunction summary_constant_divisor(): int\n{\n    return 2;\n}\n"
                    .to_string(),
            );
            rule.index_file(&file);
        }
        assert!(rule.validate_file(&use_file).unwrap().is_empty());
    }
}
