use anyhow::ensure;
use velum_differential_fuzz::{
    compare::{CaseRecord, OutcomeStatus, error_name_from_text, outcome},
    correctness::{
        CompletedOutcomeComparison, CorrectnessEvaluation, EquivalenceBasis, JsErrorClass,
        OracleEngine, OutcomeDifference, UnverifiedReason, compare_completed_outcomes, evaluate,
    },
    reference_gaps::{
        OracleDecision, OracleUnavailableReason, ReferenceAnalysis, ReferenceGapReason, analyze,
    },
};

#[test]
fn error_name_parser_extracts_nested_javascript_error() -> anyhow::Result<()> {
    let name = error_name_from_text("javascript exception: TypeError: constructor requires 'new'");
    ensure!(name == "TypeError", "unexpected error name: {name}");
    Ok(())
}

#[test]
fn error_name_parser_preserves_primary_reference_error() -> anyhow::Result<()> {
    let name = error_name_from_text("ReferenceError: \"Intl\" is not defined");
    ensure!(name == "ReferenceError", "unexpected error name: {name}");
    Ok(())
}

#[test]
fn error_name_parser_maps_lexer_errors_to_syntax_error() -> anyhow::Result<()> {
    let name = error_name_from_text(
        "lexer error at 11: invalid regular expression pattern: RegExp compile error",
    );
    ensure!(name == "SyntaxError", "unexpected error name: {name}");
    Ok(())
}

#[test]
fn successful_output_difference_is_typed_and_detailed() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "left\n", None, None);
    let oracle = outcome(OutcomeStatus::Ok, 1, "right\n", None, None);
    let comparison = compare_completed_outcomes(&velum, &oracle);
    let CompletedOutcomeComparison::Different(OutcomeDifference::SuccessfulOutput {
        velum_sha256,
        oracle_sha256,
        velum_bytes,
        oracle_bytes,
    }) = comparison
    else {
        anyhow::bail!("expected a typed successful-output difference");
    };
    ensure!(velum_sha256 != oracle_sha256);
    ensure!(velum_bytes == 5);
    ensure!(oracle_bytes == 6);
    Ok(())
}

#[test]
fn javascript_error_equivalence_records_its_limited_basis() -> anyhow::Result<()> {
    let velum = js_error("RangeError", "Velum-specific diagnostic");
    let oracle = js_error("RangeError", "V8-specific diagnostic");
    let comparison = compare_completed_outcomes(&velum, &oracle);
    ensure!(
        comparison
            == CompletedOutcomeComparison::Equivalent(EquivalenceBasis::JsErrorClass {
                class: JsErrorClass::RangeError,
            })
    );
    Ok(())
}

#[test]
fn unclassified_javascript_error_is_not_treated_as_equivalent() -> anyhow::Result<()> {
    let velum = js_error("CustomError", "left");
    let engine262 = js_error("CustomError", "right");
    let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let reference = ReferenceAnalysis {
        engine262_gaps: Vec::new(),
        oracle: OracleDecision::Engine262,
    };
    let evaluation = evaluate(&velum, &engine262, &v8, &reference, false);
    ensure!(matches!(
        evaluation,
        CorrectnessEvaluation::Unverified {
            reason: UnverifiedReason::UnclassifiedJsError {
                oracle: OracleEngine::Engine262,
                ..
            }
        }
    ));
    Ok(())
}

#[test]
fn v8_fallback_mismatch_identifies_the_selected_oracle() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = js_error("ReferenceError", "SharedArrayBuffer is not defined");
    let v8 = js_error("RangeError", "byte length should be a multiple of 8");
    let reference = ReferenceAnalysis {
        engine262_gaps: vec![ReferenceGapReason::MissingEngine262Global],
        oracle: OracleDecision::V8Fallback,
    };
    let evaluation = evaluate(&velum, &engine262, &v8, &reference, false);
    ensure!(matches!(
        evaluation,
        CorrectnessEvaluation::Mismatch {
            oracle: OracleEngine::V8Fallback,
            difference: OutcomeDifference::Status {
                velum: OutcomeStatus::Ok,
                oracle: OutcomeStatus::JsError,
            }
        }
    ));
    Ok(())
}

#[test]
fn unavailable_oracle_preserves_all_typed_reasons() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = js_error("ReferenceError", "SharedArrayBuffer is not defined");
    let v8 = js_error("RangeError", "byte length should be a multiple of 8");
    let reasons = vec![
        OracleUnavailableReason::SharedArrayBufferAlignmentConflict,
        OracleUnavailableReason::LegacyDecimalEscapeWithV8Alignment,
    ];
    let reference = ReferenceAnalysis {
        engine262_gaps: vec![ReferenceGapReason::SharedArrayBufferAlignmentConflict],
        oracle: OracleDecision::Unavailable {
            reasons: reasons.clone(),
        },
    };
    let evaluation = evaluate(&velum, &engine262, &v8, &reference, false);
    ensure!(
        evaluation
            == CorrectnessEvaluation::Unverified {
                reason: UnverifiedReason::NoReliableOracle { reasons },
            }
    );
    Ok(())
}

#[test]
fn shared_array_buffer_alignment_gap_is_auditable() -> anyhow::Result<()> {
    let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
    let engine262 = js_error("ReferenceError", "SharedArrayBuffer is not defined");
    let v8 = js_error(
        "RangeError",
        "byte length of BigInt64Array should be a multiple of 8",
    );
    let source = "new BigInt64Array(new SharedArrayBuffer(6, { maxByteLength: 6 }))";
    let reference = analyze(source, &velum, &engine262, &v8);
    ensure!(
        reference
            .engine262_gaps
            .contains(&ReferenceGapReason::SharedArrayBufferAlignmentConflict)
    );
    let OracleDecision::Unavailable { reasons } = reference.oracle else {
        anyhow::bail!("expected the ambiguous alignment case to have no reliable oracle");
    };
    ensure!(reasons.contains(&OracleUnavailableReason::SharedArrayBufferAlignmentConflict));
    Ok(())
}

#[test]
fn legacy_case_record_deserializes_without_claiming_a_verdict() -> anyhow::Result<()> {
    let record: CaseRecord = serde_json::from_value(serde_json::json!({
        "case_id": "legacy",
        "worker_pid": 1,
        "sequence": 1,
        "script_sha256": "hash",
        "script_bytes": 1,
        "classification": "match",
        "saved_script": null,
        "ratio_velum_to_v8": null,
        "velum": serialized_outcome(),
        "v8": serialized_outcome()
    }))?;
    ensure!(record.schema_version == 1);
    ensure!(matches!(
        record.correctness_evaluation,
        CorrectnessEvaluation::LegacyUnspecified
    ));
    ensure!(matches!(
        record.reference_analysis.oracle,
        OracleDecision::LegacyUnspecified
    ));
    Ok(())
}

fn js_error(name: &str, message: &str) -> velum_differential_fuzz::compare::EngineOutcome {
    outcome(
        OutcomeStatus::JsError,
        1,
        "",
        Some(name.to_owned()),
        Some(message.to_owned()),
    )
}

fn serialized_outcome() -> serde_json::Value {
    serde_json::json!({
        "status": "ok",
        "elapsed_nanos": 1,
        "stdout_sha256": "hash",
        "stdout_bytes": 0,
        "error_name": null,
        "error_message": null
    })
}
