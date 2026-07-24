use crate::compare::EngineOutcome;
use crate::reference_gap_predicates as predicates;

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
        || predicates::is_resizable_array_buffer_reference_divergence(source, velum, engine262, v8)
        || predicates::is_reference_unsupported_resource_management_syntax(source, engine262, v8)
        || predicates::is_reference_unsupported_resource_management_symbols(source, velum, engine262, v8)
        || predicates::is_engine262_missing_annex_b_string_legacy_method(source, velum, engine262, v8)
        || predicates::is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || predicates::is_annex_b_string_legacy_with_unavailable_v8_fallback(source, engine262, v8)
        || predicates::is_engine262_missing_annex_b_regexp_compile_method(source, velum, engine262)
        || predicates::is_reference_unsupported_immutable_array_buffer_method(source, velum, engine262, v8)
        || predicates::is_reference_unsupported_date_temporal_instant_method(source, velum, engine262, v8)
        || predicates::is_engine262_locale_validation_gap(source, velum, engine262, v8)
        || predicates::is_webassembly_host_api_gap(source, velum, engine262, v8)
        || predicates::is_shared_array_buffer_alignment_without_oracle(source, engine262, v8)
        || predicates::is_resizable_array_buffer_alignment_without_oracle(source, engine262, v8)
        || predicates::is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || predicates::is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_legacy_control_escape_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || predicates::is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || predicates::is_shared_array_buffer_zero_length_slice_without_oracle(source, engine262, v8)
        || predicates::is_native_function_throw_stringification_without_oracle(source, engine262, v8)
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
        || predicates::is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || predicates::is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_legacy_control_escape_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || predicates::is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || predicates::is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || predicates::is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || predicates::is_annex_b_string_legacy_with_unavailable_v8_fallback(source, engine262, v8)
        || predicates::is_shared_array_buffer_zero_length_slice_without_oracle(source, engine262, v8)
        || predicates::is_native_function_throw_stringification_without_oracle(source, engine262, v8)
        || predicates::is_fuzzilli_introspection_reference_unstable(source, engine262, v8)
        || predicates::source_contains_resource_management_symbol_access(source)
            && predicates::references_complete_equivalently(engine262, v8)
        || predicates::is_reference_missing_immutable_array_buffer_method(source, engine262, v8)
        || predicates::is_reference_missing_date_temporal_instant_method(source, engine262, v8)
        || predicates::is_v8_fallback_unavailable(v8)
    {
        return None;
    }
    Some(v8)
}
