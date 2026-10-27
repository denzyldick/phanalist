#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
use crate::file::File;
#[cfg(test)]
use crate::results::Violation;
#[cfg(test)]
use crate::verification::FindingClass;

#[cfg(test)]
pub(crate) static CODE: &str = "E0031";

#[allow(unused_imports)]
pub use crate::verification::Settings;

#[cfg(test)]
#[derive(Default)]
pub struct Rule {
    pub settings: Settings,
    pub summaries: std::sync::Mutex<crate::verification::SummaryIndex>,
}

#[cfg(test)]
impl crate::rules::Rule for Rule {
    fn get_code(&self) -> String {
        String::from(CODE)
    }

    fn description(&self) -> String {
        String::from("Formal Verification")
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

    fn remove_file(&self, path: &std::path::Path) {
        if let Ok(mut summaries) = self.summaries.lock() {
            summaries.remove_file(path);
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
        let (summaries, parents) = self
            .summaries
            .lock()
            .map(|summaries| (summaries.snapshot(), summaries.parent_map()))
            .unwrap_or_default();
        let findings = crate::verification::analyze_file_with_summaries(
            file,
            &self.settings,
            &summaries,
            &parents,
            &FindingClass::ALL,
        );
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
    fn reads_tracked_literal_array_values() {
        let violations = analyze_file_for_rule("e31/array_access_tracked.php", CODE);

        assert_eq!(violations.len(), 2);
        assert!(violations
            .iter()
            .all(|violation| violation.message.render().contains("Division by zero")));
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
    fn handles_empty_switch_defaults_and_nested_array_writes() {
        assert!(messages("e31/switch_empty_default.php").is_empty());
    }

    #[test]
    fn empty_default_in_the_middle_falls_through_to_the_next_case() {
        let messages = messages("e31/switch_middle_default.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }

    #[test]
    fn reports_nested_missing_keys_but_forgets_mutated_nested_arrays() {
        let messages = messages("e31/nested_array_keys.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Array key 'missing'"));
    }

    #[test]
    fn only_forgets_reference_captures_that_the_closure_writes() {
        let messages = messages("e31/closure_captures.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Array key 'missing'"));
    }

    #[test]
    fn uses_static_constant_and_finite_parameter_sets() {
        let messages = messages("e31/static_method_constants.php");

        assert_eq!(messages.len(), 3);
        assert!(messages
            .iter()
            .all(|message| message.contains("Modulo by zero")));
    }

    #[test]
    fn propagates_documented_numeric_and_non_empty_contracts_to_callers() {
        assert!(messages("e31/contract_propagation.php").is_empty());
    }

    #[test]
    fn propagates_object_method_contracts_to_callers() {
        let messages = messages("e31/object_method_contract.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn propagates_object_parameter_method_contracts() {
        let messages = messages("e31/object_parameter_contract.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn does_not_apply_a_base_method_contract_through_a_subclassed_parameter() {
        assert!(messages("e31/object_parameter_inheritance.php").is_empty());
    }

    #[test]
    fn uses_propagated_contracts_in_arithmetic_findings() {
        let messages = messages("e31/contract_propagation_findings.php");

        assert_eq!(messages.len(), 3);
        assert!(messages
            .iter()
            .all(|message| message.contains("Modulo by zero")));
    }

    #[test]
    fn propagates_documented_array_shapes_to_parameters_and_callers() {
        let messages = messages("e31/shape_contract_propagation.php");

        assert_eq!(messages.len(), 2);
        assert!(messages
            .iter()
            .all(|message| message.contains("Array key 'missing'")));
    }

    #[test]
    fn uses_static_range_domains_and_builtin_expression_ranges() {
        let messages = messages("e31/static_method_ranges.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Modulo by zero"));
    }

    #[test]
    fn tracks_array_writes_and_native_false_union() {
        assert!(messages("e31/array_writes_and_false_union.php").is_empty());
    }

    #[test]
    fn forgets_arrays_passed_by_reference_to_unknown_calls() {
        assert!(messages("e31/array_mutated_by_reference.php").is_empty());
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
    fn reports_unverified_string_offset_for_an_unknown_index() {
        let messages = messages("e31/string_offset_unknown.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unverified"));
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
    fn assertions_on_unknown_values_are_treated_as_assumptions() {
        assert!(messages("e31/type_predicate_unknown.php").is_empty());
    }

    #[test]
    fn match_default_throws_do_not_make_following_code_unreachable() {
        assert!(messages("e31/match_default_throw.php").is_empty());
    }

    #[test]
    fn throws_in_conditional_expressions_do_not_make_following_code_unreachable() {
        assert!(messages("e31/throw_in_short_circuit.php").is_empty());
    }

    #[test]
    fn generator_returns_are_not_checked_as_return_contracts() {
        assert!(messages("e31/generator_return.php").is_empty());
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
    fn accepts_generic_and_shaped_nullable_documented_return_types() {
        assert!(messages("e31/phpdoc_generic_nullable_return.php").is_empty());
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
    fn does_not_flag_nested_isset_on_a_missing_key() {
        assert!(messages("e31/nested_isset_missing_key.php").is_empty());
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

    #[test]
    fn unrolls_do_while_bodies() {
        let messages = messages("e31/do_while_second_iteration.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn proves_guarded_do_while_loop() {
        assert!(messages("e31/do_while_guarded.php").is_empty());
    }

    #[test]
    fn return_in_a_do_while_loop_makes_following_code_unreachable() {
        let messages = messages("e31/do_while_return_unreachable.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }

    #[test]
    fn reports_division_after_a_possibly_empty_foreach_loop() {
        let messages = messages("e31/benchmark/loop_zero_iteration.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn reports_unverified_when_a_loop_accumulator_is_unknown() {
        let messages = messages("e31/benchmark/loop_zero_iteration_unknown.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Could not verify"));
        assert!(messages[0].contains("unverified"));
    }

    #[test]
    fn proves_a_positive_guard_over_an_unknown_loop_accumulator() {
        assert!(messages("e31/benchmark/loop_zero_iteration_unknown_guarded.php").is_empty());
    }

    #[test]
    fn keeps_an_unknown_loop_accumulator_unverified_under_a_strict_guard() {
        let messages = messages("e31/benchmark/loop_zero_iteration_unknown_strict_guard.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Could not verify"));
    }

    #[test]
    fn proves_guarded_division_after_a_possibly_empty_foreach_loop() {
        assert!(messages("e31/benchmark/loop_zero_iteration_guarded.php").is_empty());
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

    #[test]
    fn proves_an_array_merge_derived_index() {
        assert!(messages("e31/array_merge_in_bounds.php").is_empty());
    }

    #[test]
    fn reports_an_array_merge_derived_index_out_of_bounds() {
        let messages = messages("e31/array_merge_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn proves_array_map_preserves_a_list_length() {
        assert!(messages("e31/array_map_in_bounds.php").is_empty());
    }

    #[test]
    fn proves_array_values_and_reverse_preserve_lengths() {
        assert!(messages("e31/array_values_in_bounds.php").is_empty());
        assert!(messages("e31/array_reverse_in_bounds.php").is_empty());
    }

    #[test]
    fn proves_a_str_repeat_derived_string_offset() {
        assert!(messages("e31/str_repeat_in_bounds.php").is_empty());
    }

    #[test]
    fn reports_a_str_repeat_derived_string_offset_out_of_bounds() {
        let messages = messages("e31/str_repeat_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("String offset"));
    }

    #[test]
    fn proves_an_array_diff_result_is_not_longer_than_its_input() {
        assert!(messages("e31/array_diff_guarded.php").is_empty());
    }

    #[test]
    fn reports_an_array_diff_result_indexed_by_the_input_length() {
        let messages = messages("e31/array_diff_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn reports_a_multi_array_map_result_indexed_past_every_input() {
        let messages = messages("e31/array_map_multi_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn reports_an_array_fill_result_indexed_by_its_count() {
        let messages = messages("e31/array_fill_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn reports_an_array_keys_result_indexed_by_the_input_length() {
        let messages = messages("e31/array_keys_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn proves_array_key_first_and_last_on_tracked_arrays() {
        assert!(messages("e31/array_key_first_last_in_bounds.php").is_empty());
    }

    #[test]
    fn proves_array_key_first_and_last_use_insertion_order() {
        assert!(messages("e31/array_key_first_last_insertion_order.php").is_empty());
    }

    #[test]
    fn proves_array_key_first_and_last_on_empty_arrays_are_null() {
        assert!(messages("e31/array_key_first_last_empty.php").is_empty());
    }

    #[test]
    fn reports_an_array_key_last_result_used_past_the_end() {
        let messages = messages("e31/array_key_first_last_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn does_not_model_builtins_with_named_arguments_out_of_order() {
        assert!(messages("e31/array_fill_named_arguments.php").is_empty());
    }

    #[test]
    fn proves_an_array_slice_result_is_not_longer_than_its_input() {
        assert!(messages("e31/array_slice_guarded.php").is_empty());
    }

    #[test]
    fn reports_an_array_slice_index_past_the_requested_length() {
        let messages = messages("e31/array_slice_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn proves_a_substr_result_is_not_longer_than_its_requested_length() {
        assert!(messages("e31/substr_guarded.php").is_empty());
    }

    #[test]
    fn reports_a_substr_index_past_the_requested_length() {
        let messages = messages("e31/substr_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("String offset"));
    }

    #[test]
    fn proves_a_str_split_result_indexes_each_character() {
        assert!(messages("e31/str_split_guarded.php").is_empty());
    }

    #[test]
    fn reports_a_str_split_index_past_the_string_length() {
        let messages = messages("e31/str_split_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn reports_an_array_chunk_index_past_the_chunk_count() {
        let messages = messages("e31/array_chunk_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("list index"));
    }

    #[test]
    fn reports_a_str_pad_index_past_the_requested_length() {
        let messages = messages("e31/str_pad_out_of_bounds.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("String offset"));
    }

    #[test]
    fn reports_list_indexes_past_the_end() {
        let messages = messages("e31/list_bounds_vulnerable.php");

        assert_eq!(messages.len(), 3);
        assert!(messages
            .iter()
            .all(|message| message.contains("list index")));
        assert!(messages.iter().any(|message| message.contains("length 0")));
    }

    #[test]
    fn proves_list_access_in_bounds() {
        assert!(messages("e31/list_bounds_safe.php").is_empty());
    }

    #[test]
    fn reports_unverified_list_access_for_an_unmodeled_index() {
        let messages = messages("e31/list_bounds_unknown.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Could not verify this list access"));
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
    use crate::rules::Rule as _;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    fn fixture_path(path: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("rules")
            .join("examples")
            .join(path)
    }

    fn messages_with_settings(path: &str, settings: Settings) -> Vec<String> {
        let path = fixture_path(path);
        let content = std::fs::read_to_string(&path).unwrap();
        let arena = mago_allocator::prelude::LocalArena::new();
        let file = crate::file::File::new(&arena, path, content);
        let rule = Rule {
            settings,
            ..Rule::default()
        };

        rule.begin_index();
        rule.index_file(&file);
        rule.finalize_index();

        rule.validate_file(&file)
            .unwrap()
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn check_types_disables_documented_type_refinements() {
        let settings = Settings {
            check_types: false,
            ..Settings::default()
        };

        assert!(messages("e31/phpdoc_param_native_positive.php").is_empty());
        assert_eq!(
            messages_with_settings("e31/phpdoc_param_native_positive.php", settings).len(),
            1
        );
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

    #[test]
    fn applies_named_phpdoc_var_refinements() {
        assert!(messages("e31/phpdoc_var_nonzero.php").is_empty());
    }

    #[test]
    fn applies_phpdoc_var_refinements_to_existing_variables() {
        assert!(messages("e31/phpdoc_var_named_existing.php").is_empty());
    }

    #[test]
    fn applies_unnamed_phpdoc_var_refinements_to_assignments() {
        assert!(messages("e31/phpdoc_var_unnamed.php").is_empty());
    }

    #[test]
    fn reports_division_without_a_phpdoc_var_refinement() {
        let messages = messages("e31/phpdoc_var_unconstrained.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn applies_phpstan_assertions_after_function_calls() {
        let path = "e31/phpdoc_assert_function.php";
        let content = std::fs::read_to_string(fixture_path(path)).unwrap();
        let arena = mago_allocator::prelude::LocalArena::new();
        let file = File::new(&arena, fixture_path(path), content);
        let rule = Rule::default();

        rule.begin_index();
        rule.index_file(&file);
        rule.finalize_index();

        assert!(rule.validate_file(&file).unwrap().is_empty());
    }

    #[test]
    fn applies_psalm_assertions_after_static_method_calls() {
        let path = "e31/phpdoc_assert_method.php";
        let content = std::fs::read_to_string(fixture_path(path)).unwrap();
        let arena = mago_allocator::prelude::LocalArena::new();
        let file = File::new(&arena, fixture_path(path), content);
        let rule = Rule::default();

        rule.begin_index();
        rule.index_file(&file);
        rule.finalize_index();

        assert!(rule.validate_file(&file).unwrap().is_empty());
    }

    #[test]
    fn reports_division_without_a_phpdoc_assertion() {
        let messages = messages("e31/phpdoc_assert_missing.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn applies_negated_phpstan_assertions() {
        let messages = messages("e31/phpdoc_assert_negated.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }

    #[test]
    fn does_not_apply_docblocks_from_other_declarations() {
        let messages = messages("e31/phpdoc_assert_unrelated.php");

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
    fn follows_non_empty_switch_case_fallthrough() {
        let messages = messages("e31/switch_body_fallthrough.php");

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

    #[test]
    fn continue_inside_switch_acts_like_break() {
        let messages = messages("e31/switch_continue_acts_like_break.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn continue_two_inside_switch_skips_the_rest_of_the_loop_body() {
        let messages = messages("e31/switch_continue_two_skips_following_statement.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }

    #[test]
    fn break_two_inside_switch_exits_the_loop_and_keeps_following_code_reachable() {
        let messages = messages("e31/switch_break_two_exits_loop.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }
}

#[cfg(test)]
mod never_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn functions_declared_never_make_following_code_unreachable() {
        let messages = messages("e31/never_return_function.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }

    #[test]
    fn methods_declared_never_make_following_code_unreachable() {
        let messages = messages("e31/never_return_method.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("unreachable"));
    }
}

#[cfg(test)]
mod try_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn analyzes_catches_even_when_the_try_block_returns() {
        let messages = messages("e31/try_catch_after_return.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn analyzes_finally_blocks_after_returns() {
        let messages = messages("e31/try_finally_after_return.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn reports_findings_inside_try_blocks() {
        let messages = messages("e31/try_catch_clean.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }
}

#[cfg(test)]
mod closure_tests {
    use crate::rules::tests::analyze_file_for_rule;

    use super::*;

    fn messages(path: &str) -> Vec<String> {
        analyze_file_for_rule(path, CODE)
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
    }

    #[test]
    fn reports_division_inside_closures() {
        let messages = messages("e31/closure_division_by_zero.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn proves_guarded_division_inside_closures() {
        assert!(messages("e31/closure_guarded_division.php").is_empty());
    }

    #[test]
    fn reports_division_inside_arrow_functions() {
        let messages = messages("e31/arrow_division_by_zero.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn analyzes_values_captured_by_closures() {
        let messages = messages("e31/closure_use_division_by_zero.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
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

    fn messages(path: &str) -> Vec<String> {
        let path = fixture(path);
        let content = fs::read_to_string(&path).unwrap();
        let arena = LocalArena::new();
        let file = File::new(&arena, path, content);
        let rule = Rule::default();

        rule.begin_index();
        rule.index_file(&file);
        rule.finalize_index();

        rule.validate_file(&file)
            .unwrap()
            .into_iter()
            .map(|violation| violation.message.render())
            .collect()
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
        assert!(violations
            .iter()
            .all(|violation| violation.message.render().contains("Division by zero")));
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
        assert!(violations
            .iter()
            .all(|violation| violation.message.render().contains("Division by zero")));
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
    fn uses_cross_file_object_method_summaries() {
        let definition_path = fixture("e31/object_method_summary_definition.php");
        let use_path = fixture("e31/object_method_summary_use.php");
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
    fn uses_inherited_method_summaries() {
        let messages = messages("e31/method_inheritance.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn uses_trait_method_summaries() {
        let messages = messages("e31/method_inheritance_trait.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn resolves_parent_method_summaries() {
        let messages = messages("e31/parent_method_call.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn resolves_parent_method_summaries_when_the_child_overrides_them() {
        let messages = messages("e31/parent_method_override.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn resolves_imported_function_aliases() {
        let messages = messages("e31/function_import_alias.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn resolves_imported_parent_classes() {
        let messages = messages("e31/method_import_alias.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn resolves_group_use_imports() {
        let messages = messages("e31/method_import_group.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn resolves_imported_static_method_calls() {
        let messages = messages("e31/method_import_call.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn resolves_parent_calls_through_imported_classes() {
        let messages = messages("e31/parent_method_import.php");

        assert_eq!(messages.len(), 1);
        assert!(messages[0].contains("Division by zero"));
    }

    #[test]
    fn prefers_overridden_method_summaries() {
        assert!(messages("e31/method_inheritance_override.php").is_empty());
    }

    #[test]
    fn uses_cross_file_inherited_method_summaries() {
        let definition_path = fixture("e31/method_inheritance_definition.php");
        let use_path = fixture("e31/method_inheritance_use.php");
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
        rule.index_file(&use_file);
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

#[cfg(test)]
mod benchmark {
    use std::collections::BTreeMap;

    use crate::rules::tests::analyze_file_for_rule;

    use super::CODE;

    const DIVISION: &str = "E0031:division-by-zero";
    const MODULO: &str = "E0032:modulo-by-zero";
    const INTDIV: &str = "E0033:intdiv-by-zero";
    const INTDIV_OVERFLOW: &str = "E0034:intdiv-overflow";
    const ARRAY_KEY: &str = "E0035:array-key";
    const ARRAY_INDEX: &str = "E0036:array-index";
    const STRING_OFFSET: &str = "E0037:string-offset";
    const NULL_METHOD: &str = "E0038:null-method-call";
    const RETURN_CONTRACT: &str = "E0040:return-contract";
    const UNREACHABLE: &str = "E0041:unreachable";
    const ASSERTION: &str = "E0042:assertion";
    const UNVERIFIED_DIVISION: &str = "E0031:unverified";
    const UNVERIFIED_ARRAY_INDEX: &str = "E0036:unverified";

    struct BenchmarkCase {
        category: &'static str,
        path: &'static str,
        expected: &'static [&'static str],
    }

    #[derive(Default, Clone, Copy)]
    struct Counts {
        true_positives: u32,
        false_positives: u32,
        false_negatives: u32,
    }

    impl Counts {
        fn add(&mut self, expected: &[&str], actual: &[String]) {
            let mut unmatched = actual.iter().map(String::as_str).collect::<Vec<_>>();
            for id in expected {
                if let Some(index) = unmatched.iter().position(|candidate| candidate == id) {
                    unmatched.remove(index);
                    self.true_positives += 1;
                } else {
                    self.false_negatives += 1;
                }
            }
            self.false_positives += unmatched.len() as u32;
        }

        fn merge(&mut self, other: Self) {
            self.true_positives += other.true_positives;
            self.false_positives += other.false_positives;
            self.false_negatives += other.false_negatives;
        }

        fn precision(self) -> f64 {
            let reported = self.true_positives + self.false_positives;
            if reported == 0 {
                1.0
            } else {
                self.true_positives as f64 / reported as f64
            }
        }

        fn recall(self) -> f64 {
            let labelled = self.true_positives + self.false_negatives;
            if labelled == 0 {
                1.0
            } else {
                self.true_positives as f64 / labelled as f64
            }
        }
    }

    const CASES: &[BenchmarkCase] = &[
        BenchmarkCase {
            category: "division",
            path: "e31/division_by_zero_literal.php",
            expected: &[DIVISION],
        },
        BenchmarkCase {
            category: "division",
            path: "e31/division_by_nonzero.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "division",
            path: "e31/division_by_parameter.php",
            expected: &[DIVISION],
        },
        BenchmarkCase {
            category: "division",
            path: "e31/division_guarded.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "division",
            path: "e31/switch_body_fallthrough.php",
            expected: &[DIVISION],
        },
        BenchmarkCase {
            category: "division",
            path: "e31/switch_division_guarded.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "division",
            path: "e31/closure_division_by_zero.php",
            expected: &[DIVISION],
        },
        BenchmarkCase {
            category: "division",
            path: "e31/closure_guarded_division.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "integer-division",
            path: "e31/modulo_by_parameter.php",
            expected: &[MODULO],
        },
        BenchmarkCase {
            category: "integer-division",
            path: "e31/intdiv_by_parameter.php",
            expected: &[INTDIV],
        },
        BenchmarkCase {
            category: "integer-division",
            path: "e31/intdiv_overflow.php",
            expected: &[INTDIV_OVERFLOW],
        },
        BenchmarkCase {
            category: "integer-division",
            path: "e31/static_method_constants.php",
            expected: &[MODULO, MODULO, MODULO],
        },
        BenchmarkCase {
            category: "arrays",
            path: "e31/array_missing_string_key.php",
            expected: &[ARRAY_KEY],
        },
        BenchmarkCase {
            category: "arrays",
            path: "e31/array_out_of_bounds.php",
            expected: &[ARRAY_KEY],
        },
        BenchmarkCase {
            category: "arrays",
            path: "e31/array_in_bounds.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "arrays",
            path: "e31/nested_array_keys.php",
            expected: &[ARRAY_KEY],
        },
        BenchmarkCase {
            category: "arrays",
            path: "e31/nested_isset_missing_key.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "arrays",
            path: "e31/isset_missing_array_key.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "lists",
            path: "e31/list_bounds_vulnerable.php",
            expected: &[ARRAY_INDEX, ARRAY_INDEX, ARRAY_INDEX],
        },
        BenchmarkCase {
            category: "lists",
            path: "e31/list_bounds_safe.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "lists",
            path: "e31/list_bounds_unknown.php",
            expected: &[UNVERIFIED_ARRAY_INDEX],
        },
        BenchmarkCase {
            category: "strings",
            path: "e31/string_offset_out_of_bounds.php",
            expected: &[STRING_OFFSET],
        },
        BenchmarkCase {
            category: "strings",
            path: "e31/string_offset_in_bounds.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "strings",
            path: "e31/string_offset_negative.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "nullability",
            path: "e31/null_method_call.php",
            expected: &[NULL_METHOD],
        },
        BenchmarkCase {
            category: "nullability",
            path: "e31/nullsafe_method_call.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "returns",
            path: "e31/return_contract_bad_string.php",
            expected: &[RETURN_CONTRACT],
        },
        BenchmarkCase {
            category: "returns",
            path: "e31/phpdoc_return_bad.php",
            expected: &[RETURN_CONTRACT],
        },
        BenchmarkCase {
            category: "returns",
            path: "e31/phpdoc_return_good.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "unreachable",
            path: "e31/unreachable_after_return.php",
            expected: &[UNREACHABLE],
        },
        BenchmarkCase {
            category: "unreachable",
            path: "e31/switch_middle_default.php",
            expected: &[UNREACHABLE],
        },
        BenchmarkCase {
            category: "unreachable",
            path: "e31/switch_empty_default.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "assertions",
            path: "e31/assertion_always_false.php",
            expected: &[ASSERTION],
        },
        BenchmarkCase {
            category: "assertions",
            path: "e31/assertion_guard.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "switch",
            path: "e31/switch_fallthrough_zero.php",
            expected: &[DIVISION],
        },
        BenchmarkCase {
            category: "integer-division",
            path: "e31/static_method_ranges.php",
            expected: &[MODULO],
        },
        BenchmarkCase {
            category: "contracts",
            path: "e31/contract_propagation.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "contracts",
            path: "e31/contract_propagation_findings.php",
            expected: &[MODULO, MODULO, MODULO],
        },
        BenchmarkCase {
            category: "contracts",
            path: "e31/shape_contract_propagation.php",
            expected: &[ARRAY_KEY, ARRAY_KEY],
        },
        BenchmarkCase {
            category: "contracts",
            path: "e31/object_method_contract.php",
            expected: &[DIVISION],
        },
        BenchmarkCase {
            category: "division",
            path: "e31/benchmark/division.php",
            expected: &[DIVISION, DIVISION, MODULO, INTDIV, INTDIV_OVERFLOW],
        },
        BenchmarkCase {
            category: "arrays",
            path: "e31/benchmark/collections.php",
            expected: &[ARRAY_KEY, ARRAY_KEY, ARRAY_KEY, STRING_OFFSET],
        },
        BenchmarkCase {
            category: "returns",
            path: "e31/benchmark/contracts.php",
            expected: &[RETURN_CONTRACT, RETURN_CONTRACT, NULL_METHOD],
        },
        BenchmarkCase {
            category: "control-flow",
            path: "e31/benchmark/control_flow.php",
            expected: &[UNREACHABLE, DIVISION, ASSERTION],
        },
        BenchmarkCase {
            category: "loops",
            path: "e31/benchmark/loop_zero_iteration.php",
            expected: &[DIVISION],
        },
        BenchmarkCase {
            category: "loops",
            path: "e31/benchmark/loop_zero_iteration_unknown.php",
            expected: &[UNVERIFIED_DIVISION],
        },
        BenchmarkCase {
            category: "loops",
            path: "e31/benchmark/loop_zero_iteration_unknown_guarded.php",
            expected: &[],
        },
        BenchmarkCase {
            category: "loops",
            path: "e31/benchmark/loop_zero_iteration_unknown_strict_guard.php",
            expected: &[UNVERIFIED_DIVISION],
        },
        BenchmarkCase {
            category: "loops",
            path: "e31/benchmark/loop_zero_iteration_guarded.php",
            expected: &[],
        },
    ];

    #[test]
    fn supported_fragment_precision_and_recall() {
        let mut by_category: BTreeMap<&'static str, Counts> = BTreeMap::new();
        let mut overall = Counts::default();

        for case in CASES {
            let actual = analyze_file_for_rule(case.path, CODE)
                .into_iter()
                .map(|violation| violation.message.id)
                .collect::<Vec<_>>();
            let mut counts = Counts::default();
            counts.add(case.expected, &actual);
            by_category.entry(case.category).or_default().merge(counts);
            overall.merge(counts);
        }

        println!("E0031 supported-fragment benchmark ({} cases)", CASES.len());
        for (category, counts) in &by_category {
            println!(
                "{category:16} tp={} fp={} fn={} precision={:.1}% recall={:.1}%",
                counts.true_positives,
                counts.false_positives,
                counts.false_negatives,
                counts.precision() * 100.0,
                counts.recall() * 100.0,
            );
        }
        println!(
            "{:16} tp={} fp={} fn={} precision={:.1}% recall={:.1}%",
            "overall",
            overall.true_positives,
            overall.false_positives,
            overall.false_negatives,
            overall.precision() * 100.0,
            overall.recall() * 100.0,
        );

        assert!(overall.true_positives > 0);
        assert!(
            overall.precision() >= 0.95,
            "precision regressed: {:.1}%",
            overall.precision() * 100.0
        );
        assert!(
            overall.recall() >= 0.90,
            "recall regressed: {:.1}%",
            overall.recall() * 100.0
        );
    }
}
