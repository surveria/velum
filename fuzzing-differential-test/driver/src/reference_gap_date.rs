use crate::compare::{EngineOutcome, OutcomeStatus};
use crate::reference_gap_predicates::outcomes_equivalent;

const DATE_SET_YEAR_METHOD: &str = "setYear";
const DATE_SET_YEAR_MISSING_ERROR: &str = "setYear is not a function";
const DATE_GET_YEAR_METHOD: &str = "getYear";
const DATE_GET_YEAR_MISSING_ERROR: &str = "getYear is not a function";
const DATE_TO_GMT_STRING_METHOD: &str = "toGMTString";
const ENGINE262_TO_GMT_STRING_RECEIVER_ERROR: &str = "Cannot convert undefined to object";

pub fn is_engine262_missing_legacy_date_method(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    outcomes_equivalent(velum, v8)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && engine262.error_message.as_deref().is_some_and(|message| {
            source_contains_set_year_reference(source)
                && message.contains(DATE_SET_YEAR_MISSING_ERROR)
                || source_contains_get_year_reference(source)
                    && message.contains(DATE_GET_YEAR_MISSING_ERROR)
        })
}

pub fn is_engine262_legacy_date_call_gap(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    outcomes_equivalent(velum, v8)
        && source_contains_to_gmt_string_reference(source)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(ENGINE262_TO_GMT_STRING_RECEIVER_ERROR))
}

fn source_contains_set_year_reference(source: &str) -> bool {
    source.contains(".setYear(")
        || source.contains("[\"setYear\"]")
        || source.contains("['setYear']")
        || source.contains(DATE_SET_YEAR_METHOD) && source.contains("setYear(")
}

fn source_contains_get_year_reference(source: &str) -> bool {
    source.contains(".getYear(")
        || source.contains("[\"getYear\"]")
        || source.contains("['getYear']")
        || source.contains(DATE_GET_YEAR_METHOD) && source.contains("getYear(")
}

fn source_contains_to_gmt_string_reference(source: &str) -> bool {
    source.contains(".toGMTString(")
        || source.contains(".toGMTString.")
        || source.contains("[\"toGMTString\"]")
        || source.contains("['toGMTString']")
        || source.contains(DATE_TO_GMT_STRING_METHOD) && source.contains("toGMTString(")
}
