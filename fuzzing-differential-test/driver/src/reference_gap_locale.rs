use crate::compare::{EngineOutcome, OutcomeStatus};
use crate::reference_gap_predicates::outcomes_equivalent;

const STRING_LOCALE_CASE_METHODS: [&str; 2] = ["toLocaleLowerCase", "toLocaleUpperCase"];

pub fn is_engine262_string_case_locale_validation_gap(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_string_locale_case_reference(source)
        && outcomes_equivalent(velum, v8)
        && !outcomes_equivalent(velum, engine262)
        && locale_validation_error(velum)
        && locale_validation_error(v8)
}

fn source_contains_string_locale_case_reference(source: &str) -> bool {
    STRING_LOCALE_CASE_METHODS
        .iter()
        .any(|method| source_contains_method_reference(source, method))
}

fn source_contains_method_reference(source: &str, method: &str) -> bool {
    source.contains(&format!(".{method}("))
        || source.contains(&format!("[\"{method}\"]"))
        || source.contains(&format!("['{method}']"))
        || source.contains(method) && source.contains(&format!("{method}("))
}

fn locale_validation_error(outcome: &EngineOutcome) -> bool {
    outcome.status == OutcomeStatus::JsError
        && outcome
            .error_name
            .as_deref()
            .is_some_and(|name| matches!(name, "RangeError" | "TypeError"))
}
