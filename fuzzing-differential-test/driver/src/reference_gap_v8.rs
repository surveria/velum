use crate::compare::{EngineOutcome, OutcomeStatus};

const MAP_GROUP_BY_MISSING_ERROR: &str = "Map.groupBy is not a function";
const MAP_GET_OR_INSERT_MISSING_ERROR: &str = "getOrInsert is not a function";
const MAP_GET_OR_INSERT_COMPUTED_MISSING_ERROR: &str = "getOrInsertComputed is not a function";
const DATE_TO_TEMPORAL_INSTANT: &str = "toTemporalInstant";
const DATE_TO_TEMPORAL_INSTANT_MISSING_ERROR: &str = "toTemporalInstant is not a function";
const ARRAY_FROM_ASYNC: &str = "fromAsync";
const ARRAY_FROM_ASYNC_MISSING_ERROR: &str = "Array.fromAsync is not a function";
const ARRAY_BUFFER_TRANSFER: &str = "transfer";
const ARRAY_BUFFER_TRANSFER_TO_FIXED_LENGTH: &str = "transferToFixedLength";
const UNDEFINED_APPLY_ERROR: &str = "Cannot read properties of undefined (reading 'apply')";

pub fn is_v8_missing_map_group_by(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(MAP_GROUP_BY_MISSING_ERROR))
}

pub fn is_v8_missing_map_get_or_insert(source: &str, v8: &EngineOutcome) -> bool {
    source_contains_map_get_or_insert_reference(source)
        && v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(is_map_get_or_insert_missing_error)
}

pub fn is_v8_missing_date_to_temporal_instant(source: &str, v8: &EngineOutcome) -> bool {
    source.contains(DATE_TO_TEMPORAL_INSTANT)
        && v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(DATE_TO_TEMPORAL_INSTANT_MISSING_ERROR))
}

pub fn is_v8_missing_array_buffer_transfer_to_fixed_length(
    source: &str,
    v8: &EngineOutcome,
) -> bool {
    source.contains(ARRAY_BUFFER_TRANSFER_TO_FIXED_LENGTH)
        && is_v8_type_error_with(v8, is_transfer_to_fixed_length_missing_error)
}

pub fn is_v8_missing_array_from_async(source: &str, v8: &EngineOutcome) -> bool {
    source_contains_array_from_async_reference(source)
        && is_v8_type_error_with(v8, |message| {
            message.contains(ARRAY_FROM_ASYNC_MISSING_ERROR)
        })
}

pub fn is_v8_missing_array_buffer_transfer(source: &str, v8: &EngineOutcome) -> bool {
    source_contains_transfer_reference(source)
        && v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("transfer is not a function"))
}

fn source_contains_transfer_reference(source: &str) -> bool {
    source.contains(".transfer(")
        || source.contains("[\"transfer\"]")
        || source.contains("['transfer']")
        || source.contains(ARRAY_BUFFER_TRANSFER)
            && source.contains("transfer()")
            && !source.contains(ARRAY_BUFFER_TRANSFER_TO_FIXED_LENGTH)
}

fn source_contains_array_from_async_reference(source: &str) -> bool {
    source.contains(".fromAsync(")
        || source.contains("[\"fromAsync\"]")
        || source.contains("['fromAsync']")
        || source.contains("Array.fromAsync")
        || source.contains(ARRAY_FROM_ASYNC) && source.contains("fromAsync(")
}

fn source_contains_map_get_or_insert_reference(source: &str) -> bool {
    source.contains(".getOrInsert(")
        || source.contains(".getOrInsertComputed(")
        || source.contains("[\"getOrInsert\"]")
        || source.contains("[\"getOrInsertComputed\"]")
        || source.contains("['getOrInsert']")
        || source.contains("['getOrInsertComputed']")
}

fn is_map_get_or_insert_missing_error(message: &str) -> bool {
    message.contains(MAP_GET_OR_INSERT_MISSING_ERROR)
        || message.contains(MAP_GET_OR_INSERT_COMPUTED_MISSING_ERROR)
}

fn is_transfer_to_fixed_length_missing_error(message: &str) -> bool {
    message.contains("transferToFixedLength is not a function")
        || message.contains(UNDEFINED_APPLY_ERROR)
}

fn is_v8_type_error_with(outcome: &EngineOutcome, predicate: impl FnOnce(&str) -> bool) -> bool {
    outcome.status == OutcomeStatus::JsError
        && outcome.error_name.as_deref() == Some("TypeError")
        && outcome.error_message.as_deref().is_some_and(predicate)
}
