use crate::compare::{EngineOutcome, OutcomeStatus};

const MAP_GROUP_BY_MISSING_ERROR: &str = "Map.groupBy is not a function";
const ARRAY_BUFFER_TRANSFER_TO_FIXED_LENGTH: &str = "transferToFixedLength";

pub fn is_v8_missing_map_group_by(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(MAP_GROUP_BY_MISSING_ERROR))
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
