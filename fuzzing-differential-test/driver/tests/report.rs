use std::{
    fs,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, ensure};
use velum_differential_fuzz::{
    compare::{CASE_RECORD_SCHEMA_VERSION, CaseRecord, OutcomeStatus, outcome},
    correctness::{CaseClassification, CaseFinding, CorrectnessEvaluation, UnverifiedReason},
    reference_gaps::{
        OracleDecision, OracleUnavailableReason, ReferenceAnalysis, ReferenceGapReason,
    },
    report::build_report,
};

#[test]
fn report_separates_unverified_cases_and_typed_reasons() -> anyhow::Result<()> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    let session = std::env::temp_dir().join(format!(
        "velum-differential-report-{}-{suffix}",
        std::process::id()
    ));
    let cases = session.join("cases");
    fs::create_dir_all(&cases)?;
    let unavailable_reason = OracleUnavailableReason::SharedArrayBufferAlignmentConflict;
    let record = CaseRecord {
        schema_version: CASE_RECORD_SCHEMA_VERSION,
        case_id: "typed-unverified".to_owned(),
        worker_pid: 1,
        sequence: 1,
        script_sha256: "script-hash".to_owned(),
        script_bytes: 1,
        classification: CaseClassification::CorrectnessUnverified,
        findings: vec![
            CaseFinding::CorrectnessUnverified,
            CaseFinding::Engine262Unsupported,
        ],
        reference_analysis: ReferenceAnalysis {
            engine262_gaps: vec![ReferenceGapReason::SharedArrayBufferAlignmentConflict],
            oracle: OracleDecision::Unavailable {
                reasons: vec![unavailable_reason],
            },
        },
        correctness_evaluation: CorrectnessEvaluation::Unverified {
            reason: UnverifiedReason::NoReliableOracle {
                reasons: vec![unavailable_reason],
            },
        },
        saved_script: None,
        saved_scripts: Vec::new(),
        ratio_velum_to_v8: None,
        velum: outcome(OutcomeStatus::Ok, 1, "", None, None),
        engine262: outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("ReferenceError".to_owned()),
            Some("SharedArrayBuffer is not defined".to_owned()),
        ),
        v8: outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("RangeError".to_owned()),
            Some("byte length should be a multiple of 8".to_owned()),
        ),
    };
    fs::write(
        cases.join("cases-1.jsonl"),
        format!("{}\n", serde_json::to_string(&record)?),
    )?;
    let report = build_report(&session, Duration::from_secs(1), "completed")?;
    let rendered = report.render();
    ensure!(rendered.contains("Correctness unverified"));
    ensure!(rendered.contains("No reliable oracle"));
    ensure!(rendered.contains("shared_array_buffer_alignment_conflict"));
    fs::remove_dir_all(&session)
        .with_context(|| format!("failed to remove test session '{}'", session.display()))?;
    Ok(())
}
