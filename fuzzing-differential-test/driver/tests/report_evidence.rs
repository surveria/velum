use std::{
    fs,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, ensure};
use velum_differential_fuzz::{
    compare::{CASE_RECORD_SCHEMA_VERSION, CaseRecord, OutcomeStatus, outcome},
    correctness::{
        CaseClassification, CorrectnessEvaluation, EquivalenceBasis, JsErrorClass, OracleEngine,
        UnverifiedReason,
    },
    reference_gaps::{OracleDecision, ReferenceAnalysis},
    report::build_report,
};

#[test]
fn report_distinguishes_observations_oracles_and_duplicate_executions() -> anyhow::Result<()> {
    let printed = record(
        "printed",
        "42\n",
        CorrectnessEvaluation::Equivalent {
            oracle: OracleEngine::Engine262,
            basis: EquivalenceBasis::SuccessfulOutputSha256,
        },
    );
    let empty = record(
        "empty",
        "",
        CorrectnessEvaluation::Equivalent {
            oracle: OracleEngine::V8Fallback,
            basis: EquivalenceBasis::SuccessfulOutputSha256,
        },
    );
    let mut error = record(
        "error",
        "before exception\n",
        CorrectnessEvaluation::Equivalent {
            oracle: OracleEngine::Engine262,
            basis: EquivalenceBasis::JsErrorClass {
                class: JsErrorClass::TypeError,
            },
        },
    );
    error.velum = outcome(
        OutcomeStatus::JsError,
        1,
        "before exception\n",
        Some("TypeError".to_owned()),
        Some("diagnostic".to_owned()),
    );
    error.engine262 = error.velum.clone();
    let mut unverified = record(
        "unverified",
        "42\n",
        CorrectnessEvaluation::Unverified {
            reason: UnverifiedReason::OracleIncomplete {
                oracle: OracleEngine::Engine262,
                status: OutcomeStatus::Timeout,
            },
        },
    );
    unverified.engine262 = outcome(OutcomeStatus::Timeout, 1, "", None, None);
    let legacy = record("legacy", "42\n", CorrectnessEvaluation::LegacyUnspecified);
    let rendered = render_records(&[printed.clone(), printed, empty, error, unverified, legacy])?;
    for (metric, expected) in [
        ("Compared scripts", "6"),
        ("Distinct script hashes", "5"),
        ("Correctness equivalent", "4"),
        ("Equivalent with Engine262", "3"),
        ("Equivalent with V8 fallback", "1"),
        ("Equal non-empty output", "2"),
        ("Both successful, no output", "1"),
        ("Same JS error class only", "1"),
        ("Correctness unverified", "1"),
        ("Legacy records without typed verdict", "1"),
    ] {
        ensure!(
            metric_value(&rendered, metric) == Some(expected),
            "unexpected report metric {metric}; expected {expected}:\n{rendered}"
        );
    }
    Ok(())
}

fn record(case_id: &str, output: &str, verdict: CorrectnessEvaluation) -> CaseRecord {
    let oracle = match verdict.oracle() {
        Some(OracleEngine::V8Fallback) => OracleDecision::V8Fallback,
        Some(OracleEngine::Engine262) => OracleDecision::Engine262,
        None => OracleDecision::LegacyUnspecified,
    };
    CaseRecord {
        schema_version: CASE_RECORD_SCHEMA_VERSION,
        case_id: case_id.to_owned(),
        worker_pid: 1,
        sequence: 1,
        script_sha256: case_id.to_owned(),
        script_bytes: 1,
        classification: if verdict.is_unverified() {
            CaseClassification::CorrectnessUnverified
        } else {
            CaseClassification::Match
        },
        findings: Vec::new(),
        reference_analysis: ReferenceAnalysis {
            engine262_gaps: Vec::new(),
            oracle,
        },
        correctness_evaluation: verdict,
        saved_script: None,
        saved_scripts: Vec::new(),
        ratio_velum_to_v8: None,
        velum: outcome(OutcomeStatus::Ok, 1, output, None, None),
        engine262: outcome(OutcomeStatus::Ok, 1, output, None, None),
        v8: outcome(OutcomeStatus::Ok, 1, output, None, None),
    }
}

fn render_records(records: &[CaseRecord]) -> anyhow::Result<String> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_nanos();
    let session = std::env::temp_dir().join(format!(
        "velum-differential-evidence-{}-{suffix}",
        std::process::id()
    ));
    let cases = session.join("cases");
    fs::create_dir_all(&cases)?;
    let mut jsonl = String::new();
    for record in records {
        jsonl.push_str(&serde_json::to_string(record)?);
        jsonl.push('\n');
    }
    fs::write(cases.join("cases-1.jsonl"), jsonl)?;
    let report = build_report(&session, Duration::from_secs(1), "completed")?;
    let rendered = report.render();
    fs::remove_dir_all(&session)
        .with_context(|| format!("failed to remove test session '{}'", session.display()))?;
    Ok(rendered)
}

fn metric_value<'a>(rendered: &'a str, metric: &str) -> Option<&'a str> {
    rendered.lines().find_map(|line| {
        let mut cells = line
            .split('|')
            .map(str::trim)
            .filter(|cell| !cell.is_empty());
        if cells.next() != Some(metric) {
            return None;
        }
        cells.next()
    })
}
