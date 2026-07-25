use crate::compare::{EngineOutcome, OutcomeStatus};
use crate::reference_gap_predicates::references_complete_equivalently;

const RESIZABLE_ARRAY_BUFFER_MARKER: &str = "maxByteLength";
const SHARED_ARRAY_BUFFER_CONSTRUCTOR: &str = "SharedArrayBuffer";
const REGEXP_COMPILE_METHOD: &str = "compile";
const PREVENT_EXTENSIONS_METHOD: &str = "preventExtensions";
const LOCALE_LOWERCASE_METHOD: &str = "toLocaleLowerCase";
const TO_LOCALE_STRING_METHOD: &str = "toLocaleString";
const INTL_LOCALE_ENTRY_ERROR: &str = "Intl locale entry is invalid";
const ENGINE262_EXPECTED_CHARACTER_ERROR: &str = "Expected a character";
const ENGINE262_UNEXPECTED_TOKEN_ERROR: &str = "Unexpected token";
const USER_CONSTRUCTOR_NEW_TARGET_MARKER: &str = "new.target";
const USER_CONSTRUCTOR_NEW_TARGET_THROW: &str = "must be called with new";
const V8_TYPED_ARRAY_ALIGNMENT_ERROR: &str = "should be a multiple of";
const TYPED_ARRAY_CONSTRUCTORS: [&str; 12] = [
    "Int8Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "Float16Array",
    "Float32Array",
    "Float64Array",
    "BigInt64Array",
    "BigUint64Array",
];

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
    (source_contains_method_reference(source, LOCALE_LOWERCASE_METHOD)
        || source_contains_method_reference(source, TO_LOCALE_STRING_METHOD))
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
    (source_contains_method_reference(source, LOCALE_LOWERCASE_METHOD)
        || source_contains_method_reference(source, TO_LOCALE_STRING_METHOD))
        && outcome_is_range_error_with(v8, is_v8_typed_array_alignment_error)
}

pub fn is_gsab_length_tracking_prevent_extensions_without_oracle(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source.contains(SHARED_ARRAY_BUFFER_CONSTRUCTOR)
        && source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
        && source_contains_method_reference(source, PREVENT_EXTENSIONS_METHOD)
        && source_contains_one_argument_typed_array_constructor(source)
        && (references_complete_equivalently(engine262, v8)
            && engine262.status == OutcomeStatus::Ok
            || engine262_regexp_syntax_gap_with_v8_ok(engine262, v8))
}

fn is_v8_typed_array_alignment_error(message: &str) -> bool {
    message.contains("byte length of") && message.contains(V8_TYPED_ARRAY_ALIGNMENT_ERROR)
}

fn engine262_regexp_syntax_gap_with_v8_ok(engine262: &EngineOutcome, v8: &EngineOutcome) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("SyntaxError")
        && engine262.error_message.as_deref().is_some_and(|message| {
            message.contains(ENGINE262_EXPECTED_CHARACTER_ERROR)
                || message.contains(ENGINE262_UNEXPECTED_TOKEN_ERROR)
        })
        && v8.status == OutcomeStatus::Ok
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

fn source_contains_one_argument_typed_array_constructor(source: &str) -> bool {
    TYPED_ARRAY_CONSTRUCTORS
        .iter()
        .any(|constructor| source_contains_one_argument_constructor(source, constructor))
}

fn source_contains_one_argument_constructor(source: &str, constructor: &str) -> bool {
    let pattern = format!("new {constructor}(");
    let mut search_start = 0;
    while let Some(relative_start) = source
        .get(search_start..)
        .and_then(|tail| tail.find(&pattern))
    {
        let argument_start = search_start
            .saturating_add(relative_start)
            .saturating_add(pattern.len());
        let Some(argument_text) = source.get(argument_start..) else {
            return false;
        };
        if let Some((argument, next_start)) = first_argument_list(argument_text, argument_start) {
            if !argument.trim().is_empty() && !argument.contains(',') {
                return true;
            }
            search_start = next_start;
        } else {
            return false;
        }
    }
    false
}

fn first_argument_list(argument_text: &str, argument_start: usize) -> Option<(&str, usize)> {
    let close = argument_text.find(')')?;
    let argument = argument_text.get(..close)?;
    let next_start = argument_start.checked_add(close)?.checked_add(1)?;
    Some((argument, next_start))
}

fn outcome_is_range_error_with(
    outcome: &EngineOutcome,
    predicate: impl FnOnce(&str) -> bool,
) -> bool {
    outcome.status == OutcomeStatus::JsError
        && outcome.error_name.as_deref() == Some("RangeError")
        && outcome.error_message.as_deref().is_some_and(predicate)
}
