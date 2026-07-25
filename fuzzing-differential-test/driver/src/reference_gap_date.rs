use crate::compare::{EngineOutcome, OutcomeStatus};
use crate::reference_gap_predicates::outcomes_equivalent;

const DATE_SET_YEAR_METHOD: &str = "setYear";
const DATE_SET_YEAR_MISSING_ERROR: &str = "setYear is not a function";

pub fn is_engine262_missing_date_set_year(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_set_year_reference(source)
        && outcomes_equivalent(velum, v8)
        && engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("TypeError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(DATE_SET_YEAR_MISSING_ERROR))
}

fn source_contains_set_year_reference(source: &str) -> bool {
    source.contains(".setYear(")
        || source.contains("[\"setYear\"]")
        || source.contains("['setYear']")
        || source.contains(DATE_SET_YEAR_METHOD) && source.contains("setYear(")
}
