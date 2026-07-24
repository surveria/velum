use crate::compare::{EngineOutcome, OutcomeStatus};

const RESIZABLE_ARRAY_BUFFER_MARKER: &str = "maxByteLength";
const RESOURCE_MANAGEMENT_KEYWORD: &str = "using";
const RESOURCE_FOR_OF_KEYWORD: &str = "of";
const SHARED_ARRAY_BUFFER_CONSTRUCTOR: &str = "SharedArrayBuffer";
const SHARED_ARRAY_BUFFER_NEW_CONSTRUCTOR: &str = "new SharedArrayBuffer";
const SHARED_ARRAY_BUFFER_SLICE_METHOD: &str = "slice";
const SYMBOL_DISPOSE_ACCESS: &str = "Symbol.dispose";
const SYMBOL_ASYNC_DISPOSE_ACCESS: &str = "Symbol.asyncDispose";
const SYNTAX_ERROR_NAME: &str = "SyntaxError";
const WEBASSEMBLY_GLOBAL_ACCESS: &str = "WebAssembly";
const V8_TYPED_ARRAY_ALIGNMENT_ERROR: &str = "should be a multiple of";
const V8_SHARED_ARRAY_BUFFER_SAME_SPECIES_ERROR: &str =
    "SharedArrayBuffer subclass returned this from species constructor";
const DATE_PROTOTYPE_TO_TEMPORAL_INSTANT_METHOD: &str = "toTemporalInstant";
const ENGINE262_STACK_OVERFLOW_ERROR: &str = "Maximum call stack size exceeded";
const ENGINE262_DECIMAL_ESCAPE_CAPTURE_GROUP_ERROR: &str = "capture groups";
const ENGINE262_EXPECTED_CHARACTER_ERROR: &str = "Expected a character";
const ENGINE262_INVALID_DECIMAL_DIGITS_ERROR: &str = "Invalid decimal digits";
const ENGINE262_INVALID_IDENTITY_ESCAPE_ERROR: &str = "Invalid identity escape";
const ENGINE262_UNEXPECTED_TOKEN_ERROR: &str = "Unexpected token";
const REGEXP_NEGATIVE_LOOKAHEAD_MARKER: &str = "(?!";
const REGEXP_POSITIVE_LOOKAHEAD_MARKER: &str = "(?=";
const REGEXP_UNESCAPED_CLOSING_BRACKET_MARKER: &str = "/]";
const FUZZILLI_STUB_MARKER: &str = "typeof fuzzilli";
const FUZZILLI_EXPLORE_MARKER: &str = "EXPLORE_ACTION";
const FUZZILLI_PROBE_MARKER: &str = "PROBING_RESULTS";
const IMMUTABLE_ARRAY_BUFFER_METHODS: [&str; 3] = [
    "sliceToImmutable",
    "transferToImmutable",
    "transferToFixedLength",
];
const SET_COMPOSITION_METHODS: [&str; 7] = [
    "difference",
    "intersection",
    "isDisjointFrom",
    "isSubsetOf",
    "isSupersetOf",
    "symmetricDifference",
    "union",
];
const ANNEX_B_STRING_LEGACY_METHODS: [&str; 16] = [
    "anchor",
    "big",
    "blink",
    "bold",
    "fixed",
    "fontcolor",
    "fontsize",
    "italics",
    "link",
    "small",
    "strike",
    "substr",
    "sub",
    "sup",
    "trimLeft",
    "trimRight",
];

pub fn outcomes_equivalent(left: &EngineOutcome, right: &EngineOutcome) -> bool {
    if left.status != right.status {
        return false;
    }
    match left.status {
        OutcomeStatus::Ok => left.stdout_sha256 == right.stdout_sha256,
        OutcomeStatus::JsError => left.error_name == right.error_name,
        OutcomeStatus::Timeout | OutcomeStatus::Crash => true,
    }
}

pub fn is_engine262_unsupported(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    is_engine262_missing_global(engine262)
        || (source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
            && !outcomes_equivalent(velum, engine262)
            && !outcomes_equivalent(engine262, v8))
        || is_reference_unsupported_resource_management_syntax(source, engine262, v8)
        || is_reference_unsupported_resource_management_symbols(source, velum, engine262, v8)
        || is_engine262_missing_annex_b_string_legacy_method(source, velum, engine262, v8)
        || is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || is_annex_b_string_legacy_with_unavailable_v8_fallback(source, engine262, v8)
        || is_engine262_missing_annex_b_regexp_compile_method(source, velum, engine262)
        || is_reference_unsupported_immutable_array_buffer_method(source, velum, engine262, v8)
        || is_reference_unsupported_date_temporal_instant_method(source, velum, engine262, v8)
        || is_engine262_locale_validation_gap(source, velum, engine262, v8)
        || is_webassembly_host_api_gap(source, velum, engine262, v8)
        || is_shared_array_buffer_alignment_without_oracle(source, engine262, v8)
        || is_resizable_array_buffer_alignment_without_oracle(source, engine262, v8)
        || is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || is_legacy_control_escape_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || is_shared_array_buffer_zero_length_slice_without_oracle(source, engine262, v8)
        || is_native_function_throw_stringification_without_oracle(source, engine262, v8)
        || is_fuzzilli_introspection_reference_unstable(source, engine262, v8)
        || (engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
            && !outcomes_equivalent(velum, engine262)
            && !outcomes_equivalent(engine262, v8))
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
    if is_reference_unsupported_resource_management_syntax(source, engine262, v8)
        || is_webassembly_host_api_without_oracle(source, engine262, v8)
        || is_shared_array_buffer_alignment_without_oracle(source, engine262, v8)
        || is_resizable_array_buffer_alignment_without_oracle(source, engine262, v8)
        || is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || is_legacy_control_escape_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
            source, engine262, v8,
        )
        || is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(source, engine262, v8)
        || is_annex_b_string_legacy_with_unavailable_v8_fallback(source, engine262, v8)
        || is_shared_array_buffer_zero_length_slice_without_oracle(source, engine262, v8)
        || is_native_function_throw_stringification_without_oracle(source, engine262, v8)
        || is_fuzzilli_introspection_reference_unstable(source, engine262, v8)
        || source_contains_resource_management_symbol_access(source)
            && references_complete_equivalently(engine262, v8)
        || is_reference_missing_immutable_array_buffer_method(source, engine262, v8)
        || is_reference_missing_date_temporal_instant_method(source, engine262, v8)
        || is_v8_fallback_unavailable(v8)
    {
        return None;
    }
    Some(v8)
}

fn is_shared_array_buffer_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(SHARED_ARRAY_BUFFER_CONSTRUCTOR)
        && source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && (is_engine262_missing_global(engine262)
            || engine262.status == OutcomeStatus::JsError
                && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn is_v8_typed_array_alignment_error(message: &str) -> bool {
    message.contains("byte length of") && message.contains(V8_TYPED_ARRAY_ALIGNMENT_ERROR)
}

fn is_resizable_array_buffer_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && (is_engine262_missing_global(engine262)
            || outcome_is_range_error_with(engine262, |message| {
                message.contains("Cannot allocate memory")
            })
            || outcome_is_engine262_stack_overflow_crash(engine262))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn outcome_is_engine262_stack_overflow_crash(outcome: &EngineOutcome) -> bool {
    outcome.status == OutcomeStatus::Crash
        && outcome.error_name.as_deref() == Some("RangeError")
        && outcome
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(ENGINE262_STACK_OVERFLOW_ERROR))
}

fn is_legacy_decimal_escape_with_v8_rab_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && source_contains_legacy_decimal_escape(source)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && engine262.error_message.as_deref().is_some_and(|message| {
            message.contains(ENGINE262_DECIMAL_ESCAPE_CAPTURE_GROUP_ERROR)
                || message.contains(ENGINE262_INVALID_IDENTITY_ESCAPE_ERROR)
        })
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn source_contains_legacy_decimal_escape(source: &str) -> bool {
    let bytes = source.as_bytes();
    bytes.windows(2).any(|window| {
        window.first() == Some(&b'\\')
            && window.get(1).is_some_and(u8::is_ascii_digit)
    })
}

fn is_engine262_invalid_decimal_digits_with_v8_rab_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(ENGINE262_INVALID_DECIMAL_DIGITS_ERROR))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn is_engine262_invalid_identity_escape_with_v8_rab_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(ENGINE262_INVALID_IDENTITY_ESCAPE_ERROR))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn is_legacy_control_escape_with_v8_rab_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && source_contains_legacy_malformed_control_escape(source)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(ENGINE262_UNEXPECTED_TOKEN_ERROR))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn source_contains_legacy_malformed_control_escape(source: &str) -> bool {
    let bytes = source.as_bytes();
    bytes.windows(3).any(|window| {
        window.first() == Some(&b'\\')
            && window.get(1) == Some(&b'c')
            && window.get(2).is_some_and(|byte| !byte.is_ascii_alphabetic())
    })
}

fn is_legacy_quantified_lookahead_with_v8_rab_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && source_contains_legacy_quantified_lookahead(source)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(ENGINE262_EXPECTED_CHARACTER_ERROR))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn source_contains_legacy_quantified_lookahead(source: &str) -> bool {
    [REGEXP_POSITIVE_LOOKAHEAD_MARKER, REGEXP_NEGATIVE_LOOKAHEAD_MARKER]
        .into_iter()
        .any(|marker| source_contains_quantified_lookahead(source, marker))
}

fn source_contains_quantified_lookahead(source: &str, marker: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative_start) = source.get(search_start..).and_then(|tail| tail.find(marker)) {
        let start = search_start.saturating_add(relative_start);
        let Some(after_marker_start) = start.checked_add(marker.len()) else {
            return false;
        };
        let Some(after_marker) = source.get(after_marker_start..) else {
            return false;
        };
        let Some(close_relative) = after_marker.find(')') else {
            return false;
        };
        let Some(after_close_start) = after_marker_start
            .checked_add(close_relative)
            .and_then(|value| value.checked_add(1))
        else {
            return false;
        };
        let Some(after_close) = source.get(after_close_start..) else {
            return false;
        };
        if after_close.as_bytes().first().is_some_and(|byte| {
            matches!(*byte, b'?' | b'*' | b'+' | b'{')
        }) {
            return true;
        }
        search_start = after_close_start;
    }
    false
}

fn is_closing_bracket_regexp_with_v8_rab_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && source.contains(REGEXP_UNESCAPED_CLOSING_BRACKET_MARKER)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(ENGINE262_UNEXPECTED_TOKEN_ERROR))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn outcome_is_range_error_with(
    outcome: &EngineOutcome,
    predicate: impl FnOnce(&str) -> bool,
) -> bool {
    outcome.status == OutcomeStatus::JsError
        && outcome.error_name.as_deref() == Some("RangeError")
        && outcome.error_message.as_deref().is_some_and(predicate)
}

fn is_shared_array_buffer_zero_length_slice_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_constructs_zero_length_shared_array_buffer(source)
        && source_contains_method_reference(source, SHARED_ARRAY_BUFFER_SLICE_METHOD)
        && !source.contains("species")
        && is_engine262_missing_global(engine262)
        && v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(V8_SHARED_ARRAY_BUFFER_SAME_SPECIES_ERROR))
}

fn source_constructs_zero_length_shared_array_buffer(source: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative_start) = source
        .get(search_start..)
        .and_then(|tail| tail.find(SHARED_ARRAY_BUFFER_NEW_CONSTRUCTOR))
    {
        let start = search_start.saturating_add(relative_start);
        let Some(after_constructor_start) =
            start.checked_add(SHARED_ARRAY_BUFFER_NEW_CONSTRUCTOR.len())
        else {
            return false;
        };
        let Some(after_constructor) = source.get(after_constructor_start..) else {
            return false;
        };
        let Some(args) = after_constructor.trim_start().strip_prefix('(') else {
            search_start = after_constructor_start;
            continue;
        };
        let args = args.trim_start();
        if args.starts_with(')') {
            return true;
        }
        if let Some(after_zero) = args.strip_prefix('0') {
            let after_zero = after_zero.trim_start();
            if after_zero.starts_with(')') || after_zero.starts_with(',') {
                return true;
            }
        }
        if let Some(after_constructor) = args.strip_prefix(SHARED_ARRAY_BUFFER_CONSTRUCTOR) {
            let after_constructor = after_constructor.trim_start();
            if after_constructor.starts_with(')') || after_constructor.starts_with(',') {
                return true;
            }
        }
        search_start = after_constructor_start;
    }
    false
}

fn is_native_function_throw_stringification_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains("throw DataView;")
        && is_engine262_missing_global(engine262)
        && v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("DataView")
}

fn is_webassembly_host_api_gap(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(WEBASSEMBLY_GLOBAL_ACCESS)
        && velum.status == OutcomeStatus::JsError
        && velum.error_name.as_deref() == Some("ReferenceError")
        && velum
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(WEBASSEMBLY_GLOBAL_ACCESS))
        && is_webassembly_host_api_without_oracle(source, engine262, v8)
}

fn is_webassembly_host_api_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(WEBASSEMBLY_GLOBAL_ACCESS)
        && (is_engine262_missing_global(engine262)
            || !references_complete_equivalently(engine262, v8))
}

fn is_fuzzilli_introspection_reference_unstable(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_fuzzilli_introspection_harness(source)
        && (is_engine262_missing_global(engine262)
            || references_complete_but_disagree(engine262, v8))
}

fn source_contains_fuzzilli_introspection_harness(source: &str) -> bool {
    source.contains(FUZZILLI_STUB_MARKER)
        && (source.contains(FUZZILLI_EXPLORE_MARKER) || source.contains(FUZZILLI_PROBE_MARKER))
}

fn references_complete_but_disagree(engine262: &EngineOutcome, v8: &EngineOutcome) -> bool {
    engine262.is_completed() && v8.is_completed() && !outcomes_equivalent(engine262, v8)
}

fn is_engine262_missing_annex_b_string_legacy_method(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && outcomes_equivalent(velum, v8)
        && ANNEX_B_STRING_LEGACY_METHODS
            .iter()
            .any(|method| source_contains_method_reference(source, method))
}

fn is_annex_b_string_legacy_with_v8_rab_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && ANNEX_B_STRING_LEGACY_METHODS
            .iter()
            .any(|method| source_contains_method_reference(source, method))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn is_annex_b_string_legacy_with_unavailable_v8_fallback(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && ANNEX_B_STRING_LEGACY_METHODS
            .iter()
            .any(|method| source_contains_method_reference(source, method))
        && is_v8_fallback_unavailable(v8)
}

fn is_engine262_missing_annex_b_regexp_compile_method(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
) -> bool {
    source_contains_method_reference(source, "compile")
        && !outcomes_equivalent(velum, engine262)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("compile"))
}

fn is_reference_unsupported_immutable_array_buffer_method(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    !outcomes_equivalent(velum, engine262)
        && is_reference_missing_immutable_array_buffer_method(source, engine262, v8)
}

fn is_reference_unsupported_date_temporal_instant_method(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    !outcomes_equivalent(velum, engine262)
        && is_reference_missing_date_temporal_instant_method(source, engine262, v8)
}

fn is_reference_missing_immutable_array_buffer_method(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    references_complete_equivalently(engine262, v8)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && IMMUTABLE_ARRAY_BUFFER_METHODS
            .iter()
            .any(|method| source_contains_method_reference(source, method))
}

fn is_reference_missing_date_temporal_instant_method(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_method_reference(source, DATE_PROTOTYPE_TO_TEMPORAL_INSTANT_METHOD)
        && references_complete_equivalently(engine262, v8)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
}

fn is_engine262_locale_validation_gap(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_locale_sensitive_call(source)
        && outcomes_equivalent(velum, v8)
        && !outcomes_equivalent(velum, engine262)
        && velum.status == OutcomeStatus::JsError
        && velum.error_name.as_deref() == Some("RangeError")
}

fn source_contains_locale_sensitive_call(source: &str) -> bool {
    source.contains("Intl.")
        || source.contains("Intl[")
        || source_contains_method_reference(source, "toLocaleString")
        || source_contains_method_reference(source, "toLocaleDateString")
        || source_contains_method_reference(source, "toLocaleTimeString")
}

fn source_contains_method_reference(source: &str, method: &str) -> bool {
    source_contains_dot_property(source, method)
        || source.contains(&format!("[\"{method}\"]"))
        || source.contains(&format!("['{method}']"))
}

fn source_contains_dot_property(source: &str, method: &str) -> bool {
    let Some(pattern_len) = method.len().checked_add(1) else {
        return false;
    };
    source
        .as_bytes()
        .windows(pattern_len)
        .enumerate()
        .any(|(start, window)| {
            let Some(after_start) = start.checked_add(pattern_len) else {
                return false;
            };
            let next = source
                .get(after_start..)
                .and_then(|suffix| suffix.chars().next());
            window.first() == Some(&b'.')
                && window.get(1..) == Some(method.as_bytes())
                && !next.is_some_and(is_ascii_identifier_part)
        })
}

fn is_reference_unsupported_resource_management_syntax(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    references_reject_as_syntax(engine262, v8) && source_contains_resource_management_syntax(source)
}

fn is_reference_unsupported_resource_management_symbols(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_resource_management_symbol_access(source)
        && !outcomes_equivalent(velum, engine262)
        && (references_complete_equivalently(engine262, v8) || outcomes_equivalent(velum, v8))
}

fn references_reject_as_syntax(engine262: &EngineOutcome, v8: &EngineOutcome) -> bool {
    engine262.status == OutcomeStatus::JsError
        && v8.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && v8.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
}

fn references_complete_equivalently(engine262: &EngineOutcome, v8: &EngineOutcome) -> bool {
    engine262.is_completed() && v8.is_completed() && outcomes_equivalent(engine262, v8)
}

fn source_contains_resource_management_symbol_access(source: &str) -> bool {
    source.contains(SYMBOL_DISPOSE_ACCESS) || source.contains(SYMBOL_ASYNC_DISPOSE_ACCESS)
}

pub(crate) fn source_contains_resource_management_syntax(source: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative_start) = source
        .get(search_start..)
        .and_then(|tail| tail.find(RESOURCE_MANAGEMENT_KEYWORD))
    {
        let start = search_start.saturating_add(relative_start);
        let end = start.saturating_add(RESOURCE_MANAGEMENT_KEYWORD.len());
        if is_keyword_boundary(source, start, end)
            && resource_binding_follows_using(source.get(end..).unwrap_or_default())
        {
            return true;
        }
        search_start = end;
    }
    false
}

fn is_keyword_boundary(source: &str, start: usize, end: usize) -> bool {
    let previous = source
        .get(..start)
        .and_then(|prefix| prefix.chars().next_back());
    let next = source.get(end..).and_then(|suffix| suffix.chars().next());
    !previous.is_some_and(is_ascii_identifier_part) && !next.is_some_and(is_ascii_identifier_part)
}

fn resource_binding_follows_using(rest: &str) -> bool {
    let tail = rest.trim_start();
    let Some(first) = tail.chars().next() else {
        return false;
    };
    if !is_ascii_identifier_start(first) {
        return false;
    }
    let name_end = tail
        .char_indices()
        .find_map(|(index, value)| (!is_ascii_identifier_part(value)).then_some(index))
        .unwrap_or(tail.len());
    let Some(after_name) = tail.get(name_end..) else {
        return false;
    };
    let after_name = after_name.trim_start();
    after_name.starts_with('=') || starts_with_word(after_name, RESOURCE_FOR_OF_KEYWORD)
}

fn starts_with_word(source: &str, word: &str) -> bool {
    let Some(after) = source.get(word.len()..) else {
        return false;
    };
    source.starts_with(word) && !after.chars().next().is_some_and(is_ascii_identifier_part)
}

const fn is_ascii_identifier_start(value: char) -> bool {
    value == '_' || value == '$' || value.is_ascii_alphabetic()
}

const fn is_ascii_identifier_part(value: char) -> bool {
    is_ascii_identifier_start(value) || value.is_ascii_digit()
}

fn is_engine262_missing_global(engine262: &EngineOutcome) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("ReferenceError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(is_engine262_missing_global_message)
}

fn is_engine262_missing_global_message(message: &str) -> bool {
    message.contains("\"Atomics\" is not defined")
        || message.contains("Atomics is not defined")
        || message.contains("\"Intl\" is not defined")
        || message.contains("Intl is not defined")
        || message.contains("\"SharedArrayBuffer\" is not defined")
        || message.contains("SharedArrayBuffer is not defined")
        || message.contains("\"Temporal\" is not defined")
        || message.contains("Temporal is not defined")
}

fn is_v8_fallback_unavailable(v8: &EngineOutcome) -> bool {
    is_v8_missing_global(v8)
        || is_v8_missing_typed_array_base64_or_hex(v8)
        || is_v8_missing_math_f16round(v8)
        || is_v8_missing_set_composition(v8)
}

fn is_v8_missing_global(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("ReferenceError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(is_v8_missing_global_message)
}

fn is_v8_missing_global_message(message: &str) -> bool {
    message.contains("Iterator is not defined")
        || message.contains("AsyncIterator is not defined")
        || message.contains("DisposableStack is not defined")
        || message.contains("AsyncDisposableStack is not defined")
        || message.contains("SuppressedError is not defined")
        || message.contains("Temporal is not defined")
        || message.contains("Float16Array is not defined")
}

fn is_v8_missing_typed_array_base64_or_hex(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(is_v8_missing_typed_array_base64_or_hex_message)
}

fn is_v8_missing_typed_array_base64_or_hex_message(message: &str) -> bool {
    message.contains("toBase64 is not a function")
        || message.contains("fromBase64 is not a function")
        || message.contains("setFromBase64 is not a function")
        || message.contains("toHex is not a function")
        || message.contains("fromHex is not a function")
        || message.contains("setFromHex is not a function")
}

fn is_v8_missing_math_f16round(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("Math.f16round is not a function"))
}

fn is_v8_missing_set_composition(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8.error_message.as_deref().is_some_and(|message| {
            SET_COMPOSITION_METHODS
                .iter()
                .any(|method| message.contains(&format!("{method} is not a function")))
        })
}
