use crate::compare::{EngineOutcome, OutcomeStatus};

const MAP_GROUP_BY_MISSING_ERROR: &str = "Map.groupBy is not a function";
const MAP_GET_OR_INSERT_MISSING_ERROR: &str = "getOrInsert is not a function";
const MAP_GET_OR_INSERT_COMPUTED_MISSING_ERROR: &str = "getOrInsertComputed is not a function";
const ARRAY_BUFFER_TRANSFER: &str = "transfer";
const ARRAY_BUFFER_TRANSFER_TO_FIXED_LENGTH: &str = "transferToFixedLength";

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

pub fn is_v8_missing_array_buffer_transfer_to_fixed_length(
    source: &str,
    v8: &EngineOutcome,
) -> bool {
    source.contains(ARRAY_BUFFER_TRANSFER_TO_FIXED_LENGTH)
        && v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("transferToFixedLength is not a function"))
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
