use crate::compare::{EngineOutcome, OutcomeStatus};

const MAP_GROUP_BY_MISSING_ERROR: &str = "Map.groupBy is not a function";

pub fn is_v8_missing_map_group_by(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("TypeError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(MAP_GROUP_BY_MISSING_ERROR))
}
