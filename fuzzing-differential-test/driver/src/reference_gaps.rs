use crate::compare::EngineOutcome;
use crate::reference_gap_date as date;
use crate::reference_gap_globals as globals;
use crate::reference_gap_predicates as predicates;
use crate::reference_gap_rab as rab;
use crate::reference_gap_syntax as syntax_gaps;
use crate::reference_gap_v8 as v8_gaps;

pub use crate::reference_gap_predicates::outcomes_equivalent;
#[cfg(test)]
pub use crate::reference_gap_predicates::source_contains_resource_management_syntax;

pub fn is_engine262_unsupported(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    predicates::is_engine262_missing_global(engine262)
        || globals::is_engine262_missing_annex_b_escape_global(source, engine262, v8)
        || predicates::is_resizable_array_buffer_reference_divergence(source, velum, engine262, v8)
        || syntax_gaps::is_engine262_super_property_syntax_gap(source, velum, engine262, v8)
        || predicates::is_reference_unsupported_resource_management_syntax(source, engine262, v8)
        || predicates::is_reference_unsupported_resource_management_symbols(
            source, velum, engine262, v8,
        )
        || predicates::is_engine262_missing_annex_b_string_legacy_method(
            source, velum, engine262, v8,
        )
        || predicates::is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_annex_b_string_legacy_with_unavailable_v8_fallback(source, engine262, v8)
        || predicates::is_engine262_missing_annex_b_regexp_compile_method(source, velum, engine262)
        || rab::is_regexp_compile_with_v8_alignment_without_oracle(source, engine262, v8)
        || predicates::is_reference_unsupported_immutable_array_buffer_method(
            source, velum, engine262, v8,
        )
        || predicates::is_immutable_array_buffer_method_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || date::is_engine262_missing_legacy_date_method(source, velum, engine262, v8)
        || predicates::is_reference_unsupported_date_temporal_instant_method(
            source, velum, engine262, v8,
        )
        || predicates::is_engine262_locale_validation_gap(source, velum, engine262, v8)
        || predicates::is_engine262_template_literal_octal_escape_gap(source, velum, engine262, v8)
        || rab::is_locale_validation_gap_with_v8_alignment(source, velum, v8)
        || predicates::is_webassembly_host_api_gap(source, velum, engine262, v8)
        || predicates::is_shared_array_buffer_alignment_without_oracle(source, engine262, v8)
        || predicates::is_resizable_array_buffer_alignment_without_oracle(source, engine262, v8)
        || predicates::is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_engine262_invalid_quantifier_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || rab::is_user_constructor_throw_with_v8_alignment_without_oracle(source, engine262, v8)
        || predicates::is_legacy_control_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_shared_array_buffer_zero_length_slice_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_native_function_throw_stringification_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_fuzzilli_introspection_reference_unstable(source, engine262, v8)
        || predicates::is_engine262_syntax_error_reference_divergence(velum, engine262, v8)
}

pub fn correctness_oracle<'a>(
    source: &str,
    engine262: &'a EngineOutcome,
    v8: &'a EngineOutcome,
    engine262_unsupported: bool,
) -> Option<&'a EngineOutcome> {
    if !engine262_unsupported {
        return Some(engine262);
    }
    if predicates::is_reference_unsupported_resource_management_syntax(source, engine262, v8)
        || predicates::is_webassembly_host_api_without_oracle(source, engine262, v8)
        || predicates::is_shared_array_buffer_alignment_without_oracle(source, engine262, v8)
        || predicates::is_resizable_array_buffer_alignment_without_oracle(source, engine262, v8)
        || predicates::is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_engine262_invalid_quantifier_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || rab::is_user_constructor_throw_with_v8_alignment_without_oracle(source, engine262, v8)
        || predicates::is_legacy_control_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || rab::is_regexp_compile_with_v8_alignment_without_oracle(source, engine262, v8)
        || rab::is_locale_validation_with_v8_alignment_without_oracle(source, v8)
        || predicates::is_annex_b_string_legacy_with_unavailable_v8_fallback(source, engine262, v8)
        || predicates::is_shared_array_buffer_zero_length_slice_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_native_function_throw_stringification_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_fuzzilli_introspection_reference_unstable(source, engine262, v8)
        || predicates::source_contains_resource_management_symbol_access(source)
            && predicates::references_complete_equivalently(engine262, v8)
        || predicates::is_reference_missing_immutable_array_buffer_method(source, engine262, v8)
        || predicates::is_immutable_array_buffer_method_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_reference_missing_date_temporal_instant_method(source, engine262, v8)
        || v8_gaps::is_v8_missing_map_group_by(v8)
        || v8_gaps::is_v8_missing_map_get_or_insert(source, v8)
        || v8_gaps::is_v8_missing_date_to_temporal_instant(source, v8)
        || v8_gaps::is_v8_missing_array_from_async(source, v8)
        || v8_gaps::is_v8_missing_array_buffer_transfer(source, v8)
        || v8_gaps::is_v8_missing_array_buffer_transfer_to_fixed_length(source, v8)
        || predicates::is_v8_fallback_unavailable(v8)
    {
        return None;
    }
    Some(v8)
}
