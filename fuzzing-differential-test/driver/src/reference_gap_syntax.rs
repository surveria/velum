use crate::{
    compare::{EngineOutcome, OutcomeStatus},
    reference_gap_predicates::outcomes_equivalent,
};

pub fn is_engine262_super_property_syntax_gap(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    source_contains_super_property_access(source)
        && engine262.status == OutcomeStatus::Ok
        && velum.status == OutcomeStatus::JsError
        && velum.error_name.as_deref() == Some("SyntaxError")
        && v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("SyntaxError")
        && outcomes_equivalent(velum, v8)
}

fn source_contains_super_property_access(source: &str) -> bool {
    source.contains("super[") || source.contains("super.")
}
