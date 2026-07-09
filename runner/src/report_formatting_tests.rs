use std::time::Duration;

use super::{
    CaseRow, CorpusReport, CorpusStats, FeatureAreaStats, FullReport, STATUS_PASSED, benchmarks,
    coverage_percent, feature_area_rows_with_limit, jetstream, report_metadata,
    report_rendering::render_timing_tsv,
    report_schema::{EnvironmentInfo, ReportDocument, RunConfiguration},
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn formats_small_coverage_with_four_decimals() -> TestResult {
    ensure_text(&coverage_percent(4, 53_683), "0.0074%")
}

#[test]
fn formats_feature_area_progress_row() -> TestResult {
    let mut stats = FeatureAreaStats::new("language/statements".to_owned());
    stats.record_manifest_enabled();
    stats.record_manifest_enabled();
    stats.record_passed();
    stats.record_failed();
    stats.record_skipped("not enabled yet: Test262 language cases".to_owned());
    stats.record_skipped("not enabled yet: Test262 language cases".to_owned());
    stats.record_skipped("requires async support".to_owned());

    let rows = feature_area_rows_with_limit(vec![stats], 10);
    let Some(row) = rows.first() else {
        return Err("expected one feature area row".into());
    };

    ensure_text(&row.feature_area, "language/statements")?;
    ensure_usize(row.total, 5)?;
    ensure_usize(row.executed, 2)?;
    ensure_text(&row.passed, "1 ✅ passed")?;
    ensure_text(&row.failed, "1 ❌ failed")?;
    ensure_text(&row.skipped, "3 🟡 skipped")?;
    ensure_text(&row.pass_rate, "50.00%")?;
    ensure_usize(row.manifest_enabled, 2)?;
    ensure_text(
        &row.top_skip_reason,
        "2: not enabled yet: Test262 language cases",
    )
}

#[test]
fn compacts_feature_area_rows_after_limit() -> TestResult {
    let mut first = FeatureAreaStats::new("built-ins/Array".to_owned());
    first.record_passed();
    first.record_passed();

    let mut second = FeatureAreaStats::new("language/expressions".to_owned());
    second.record_failed();

    let rows = feature_area_rows_with_limit(vec![second, first], 1);
    ensure_usize(rows.len(), 2)?;
    let Some(first_row) = rows.first() else {
        return Err("expected first feature row".into());
    };
    ensure_text(&first_row.feature_area, "built-ins/Array")?;
    let Some(other_row) = rows.last() else {
        return Err("expected compacted feature row".into());
    };
    ensure_text(&other_row.feature_area, "other feature areas")?;
    ensure_usize(other_row.total, 1)?;
    ensure_text(&other_row.failed, "1 ❌ failed")
}

#[test]
fn report_metadata_includes_build_versions() -> TestResult {
    let metadata = report_metadata::RunMetadata::from_env();
    let rendered = report_metadata::render_section(&metadata).join("\n");

    ensure_contains(&rendered, "Engine version")?;
    ensure_contains(&rendered, "Engine build commit")?;
    ensure_contains(&rendered, "Runner version")?;
    ensure_contains(&rendered, "Runner build commit")
}

#[test]
fn timing_tsv_lists_case_rows_and_sanitizes_fields() -> TestResult {
    let report = FullReport {
        metadata: report_metadata::RunMetadata::from_env(),
        corpora: vec![CorpusReport {
            name: "Unit corpus",
            required: true,
            stats: CorpusStats {
                total: 1,
                passed: 1,
                failed: 0,
                skipped: 0,
            },
            rows: vec![CaseRow {
                case: "case-1".to_owned(),
                status: STATUS_PASSED.to_owned(),
                source: "tests/example.js".to_owned(),
                detail: "line one\nline two\tfield".to_owned(),
                elapsed: Duration::from_micros(1_250),
            }],
            skip_reasons: Vec::new(),
            feature_areas: Vec::new(),
            elapsed: Duration::from_micros(1_250),
        }],
        benchmarks: benchmarks::BenchmarkReport {
            rows: Vec::new(),
            measured: 0,
            in_process_measured: 0,
            failed: 0,
            invalid: 0,
            skipped: 0,
            over_latency_budget: 0,
            over_memory_budget: 0,
            elapsed: Duration::ZERO,
        },
        jetstream: jetstream::JetStreamReport {
            rows: Vec::new(),
            measured: 0,
            failed: 0,
            invalid: 0,
            skipped: 0,
            over_latency_budget: 0,
            elapsed: Duration::ZERO,
        },
        elapsed: Duration::from_micros(1_250),
    };

    let report = ReportDocument::from_run(
        report,
        EnvironmentInfo::capture(),
        RunConfiguration::capture(false, false),
    )?;
    let tsv = render_timing_tsv(&report);
    ensure_contains(&tsv, "kind\tphase\tcase\tstatus")?;
    ensure_contains(
        &tsv,
        "test\tUnit corpus\tcase-1\t✅ passed\ttests/example.js\t\t1.250000\t\t\tline one line two field",
    )
}

fn ensure_text(actual: &str, expected: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected '{expected}', got '{actual}'").into())
}

fn ensure_contains(actual: &str, expected: &str) -> TestResult {
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected text to contain '{expected}'").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
