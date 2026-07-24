use crate::compare::{EngineOutcome, OutcomeStatus};

const RESIZABLE_ARRAY_BUFFER_MARKER: &str = "maxByteLength";
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

fn is_v8_typed_array_alignment_error(message: &str) -> bool {
    message.contains("byte length of") && message.contains(V8_TYPED_ARRAY_ALIGNMENT_ERROR)
}

fn outcome_is_range_error_with(
    outcome: &EngineOutcome,
    predicate: impl FnOnce(&str) -> bool,
) -> bool {
    outcome.status == OutcomeStatus::JsError
        && outcome.error_name.as_deref() == Some("RangeError")
        && outcome.error_message.as_deref().is_some_and(predicate)
}
