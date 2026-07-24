use crate::compare::{EngineOutcome, OutcomeStatus};

const RESIZABLE_ARRAY_BUFFER_MARKER: &str = "maxByteLength";
const REGEXP_COMPILE_METHOD: &str = "compile";
const LOCALE_LOWERCASE_METHOD: &str = "toLocaleLowerCase";
const INTL_LOCALE_ENTRY_ERROR: &str = "Intl locale entry is invalid";
const USER_CONSTRUCTOR_NEW_TARGET_MARKER: &str = "new.target";
const USER_CONSTRUCTOR_NEW_TARGET_THROW: &str = "must be called with new";
const V8_TYPED_ARRAY_ALIGNMENT_ERROR: &str = "should be a multiple of";

pub fn is_user_constructor_throw_with_v8_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && source.contains(USER_CONSTRUCTOR_NEW_TARGET_MARKER)
        && source.contains(USER_CONSTRUCTOR_NEW_TARGET_THROW)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("Error")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(USER_CONSTRUCTOR_NEW_TARGET_THROW))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

pub fn is_regexp_compile_with_v8_alignment_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_method_reference(source, REGEXP_COMPILE_METHOD)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(REGEXP_COMPILE_METHOD))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

pub fn is_locale_validation_gap_with_v8_alignment(
    source: &str,
    velum: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_method_reference(source, LOCALE_LOWERCASE_METHOD)
        && velum.status == OutcomeStatus::JsError
        && velum.error_name.as_deref() == Some("TypeError")
        && velum
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(INTL_LOCALE_ENTRY_ERROR))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

pub fn is_locale_validation_with_v8_alignment_without_oracle(
    source: &str,
    v8: &EngineOutcome,
) -> bool {
    source_contains_method_reference(source, LOCALE_LOWERCASE_METHOD)
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

fn is_v8_typed_array_alignment_error(message: &str) -> bool {
    message.contains("byte length of") && message.contains(V8_TYPED_ARRAY_ALIGNMENT_ERROR)
}

fn source_contains_method_reference(source: &str, method: &str) -> bool {
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

const fn is_ascii_identifier_part(value: char) -> bool {
    value == '_' || value == '$' || value.is_ascii_alphanumeric()
}

fn outcome_is_range_error_with(
    outcome: &EngineOutcome,
    predicate: impl FnOnce(&str) -> bool,
) -> bool {
    outcome.status == OutcomeStatus::JsError
        && outcome.error_name.as_deref() == Some("RangeError")
        && outcome.error_message.as_deref().is_some_and(predicate)
}
