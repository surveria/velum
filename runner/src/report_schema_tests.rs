use std::time::Duration;

use crate::{
    CaseRow, CorpusReport, CorpusStats, FullReport, STATUS_PASSED, benchmarks, jetstream,
    report_metadata,
    report_rendering::{render_report, render_timing_tsv},
    report_schema::{
        BenchmarkConfiguration, BenchmarkStatus, CaseCounts, CaseDetailCoverage,
        DetailCompleteness, DetailLevel, EnvironmentInfo, ReportDocument, ReportSummary,
        RunConfiguration, SCHEMA_VERSION, SuiteStatus,
    },
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn full_yaml_round_trip_preserves_typed_report() -> TestResult {
    let report = sample_document()?;
    let yaml = serde_yaml_ng::to_string(&report)?;
    ensure_contains(&yaml, "schema_version: 1")?;
    ensure_contains(&yaml, "detail_level: full")?;
    ensure_contains(&yaml, "report_mode: full")?;
    ensure_contains(&yaml, "jetstream: disabled")?;
    ensure_contains(&yaml, "duration_ns: 7000000")?;
    ensure_contains(&yaml, "status: tracked_exception")?;

    let decoded: ReportDocument = serde_yaml_ng::from_str(&yaml)?;
    decoded.validate()?;
    ensure_bool(decoded == report, "full YAML round-trip changed the report")
}

#[test]
fn summary_yaml_omits_large_case_details() -> TestResult {
    let report = sample_document()?;
    let summary = report.summary();
    let yaml = serde_yaml_ng::to_string(&summary)?;
    ensure_contains(&yaml, "detail_level: summary")?;
    ensure_not_contains(&yaml, "case-1")?;
    ensure_not_contains(&yaml, "line one")?;

    let decoded: ReportSummary = serde_yaml_ng::from_str(&yaml)?;
    decoded.validate()?;
    let Some(suite) = decoded.suites.first() else {
        return Err("expected one suite summary".into());
    };
    ensure_u64(suite.counts.total, 1)?;
    ensure_u64(suite.counts.passed, 1)
}

#[test]
fn typed_rows_keep_numeric_durations_and_statuses() -> TestResult {
    let report = sample_document()?;
    let Some(suite) = report.suites.first() else {
        return Err("expected one suite".into());
    };
    let Some(case) = suite.cases.first() else {
        return Err("expected one case".into());
    };
    ensure_u64(case.duration_ns, 1_250_000)?;

    let Some(benchmark) = report.benchmarks.rows.first() else {
        return Err("expected one benchmark".into());
    };
    ensure_bool(
        benchmark.status == BenchmarkStatus::TrackedException,
        "expected typed benchmark status",
    )?;
    ensure_optional_u64(benchmark.case_duration_ns, Some(5_000_000))?;
    ensure_optional_u64(benchmark.engine.wall_duration_ns, Some(2_000_000))?;
    ensure_optional_u64(benchmark.engine.median_duration_ns, Some(1_250_000))?;
    ensure_optional_u64(benchmark.latency_ratio_centi_units, Some(125))
}

#[test]
fn schema_validation_rejects_unknown_version_and_detail_level() -> TestResult {
    let report = sample_document()?;
    let mut summary = report.summary();
    summary.schema_version = SCHEMA_VERSION.saturating_add(1);
    ensure_bool(
        summary.validate().is_err(),
        "unknown schema version must be rejected",
    )?;

    summary.schema_version = SCHEMA_VERSION;
    summary.detail_level = DetailLevel::Full;
    ensure_bool(
        summary.validate().is_err(),
        "summary must reject a full detail-level marker",
    )
}

#[test]
fn schema_validation_rejects_inconsistent_suite_counts() -> TestResult {
    let report = sample_document()?;
    let mut summary = report.summary();
    let Some(suite) = summary.suites.first_mut() else {
        return Err("expected one suite summary".into());
    };
    suite.counts.total = suite.counts.total.saturating_add(1);
    ensure_bool(
        summary.validate().is_err(),
        "inconsistent suite counts must be rejected",
    )
}

#[test]
fn schema_validation_accepts_explicit_partial_case_details() -> TestResult {
    let mut report = sample_document()?;
    let Some(suite) = report.suites.first_mut() else {
        return Err("expected one suite".into());
    };
    suite.cases.clear();
    suite.summary.status = SuiteStatus::Passed;
    suite.summary.counts = CaseCounts {
        total: 100,
        executed: 100,
        passed: 100,
        failed: 0,
        skipped: 0,
    };
    suite.summary.case_details = CaseDetailCoverage {
        completeness: DetailCompleteness::Partial,
        recorded_rows: 0,
        omitted_rows: 100,
    };
    report.validate()?;
    report.summary().validate()?;
    Ok(())
}

#[test]
fn structured_report_renders_compatible_markdown_and_tsv() -> TestResult {
    let report = sample_document()?;
    let markdown = render_report(&report);
    ensure_contains(&markdown, "# rs-quickjs Test Report")?;
    ensure_contains(&markdown, "## Unit corpus")?;
    ensure_contains(&markdown, "array-index")?;
    ensure_contains(&markdown, "1.25x")?;

    let tsv = render_timing_tsv(&report);
    ensure_contains(&tsv, "kind\tphase\tcase\tstatus")?;
    ensure_contains(
        &tsv,
        "test\tUnit corpus\tcase-1\t✅ passed\ttests/example.js\t\t1.250000",
    )?;
    ensure_contains(
        &tsv,
        "benchmark\tBenchmarks\tarray-index\t🟡 tracked exception",
    )
}

pub fn sample_document() -> Result<ReportDocument, anyhow::Error> {
    let report = FullReport {
        metadata: report_metadata::RunMetadata::default(),
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
                detail: "line one\nline two".to_owned(),
                elapsed: Duration::from_micros(1_250),
            }],
            skip_reasons: Vec::new(),
            feature_areas: Vec::new(),
            elapsed: Duration::from_micros(1_250),
        }],
        benchmarks: benchmarks::BenchmarkReport {
            rows: vec![benchmarks::BenchmarkRow {
                benchmark: "array-index".to_owned(),
                status: "🟡 tracked exception".to_owned(),
                source: "tests/benchmarks/array_index.js".to_owned(),
                iterations: 30,
                case_elapsed: "5.00 ms".to_owned(),
                rsqjs_measure: "2.00 ms".to_owned(),
                quickjs_measure: "3.00 ms".to_owned(),
                rsqjs_eval: "1.25 ms".to_owned(),
                quickjs_eval: "1.00 ms".to_owned(),
                latency_ratio: "1.25x".to_owned(),
                latency_budget: "🟡 > 1.00x".to_owned(),
                memory_ratio: "-".to_owned(),
                rsqjs_cv: "2.0%".to_owned(),
                quickjs_cv: "1.0%".to_owned(),
                quality: "✅ valid".to_owned(),
                detail: "sequential benchmark completed".to_owned(),
            }],
            measured: 1,
            in_process_measured: 1,
            failed: 0,
            invalid: 0,
            skipped: 0,
            over_latency_budget: 1,
            over_memory_budget: 0,
            elapsed: Duration::from_millis(5),
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
        elapsed: Duration::from_millis(7),
    };
    ReportDocument::from_run(report, sample_environment(), sample_configuration())
}

fn sample_environment() -> EnvironmentInfo {
    EnvironmentInfo {
        operating_system: "linux".to_owned(),
        architecture: "x86_64".to_owned(),
        available_parallelism: 16,
        build_profile: "test".to_owned(),
        kernel_release: Some("test-kernel".to_owned()),
        cpu_model: Some("test-cpu".to_owned()),
        cpu_affinity: Some("0-15".to_owned()),
        scaling_governor: Some("performance".to_owned()),
    }
}

fn sample_configuration() -> RunConfiguration {
    RunConfiguration {
        report_mode: crate::report_schema::ReportMode::Full,
        jetstream: crate::report_schema::FeatureSelection::Disabled,
        quickjs_differential: crate::report_schema::InputAvailability::Configured,
        test262: crate::report_schema::InputAvailability::Configured,
        test262_mode: crate::report_schema::Test262Mode::Full,
        test262_path_filters: Vec::new(),
        test262_flag_filters: Vec::new(),
        benchmark_filter: None,
        benchmark: BenchmarkConfiguration {
            reference_quickjs_compiled: true,
            warmup_duration_ns: 150_000_000,
            minimum_sample_duration_ns: 500_000_000,
            samples: 10,
            minimum_operation_duration_ns: 1_000_000,
            maximum_cv_permille: 100,
            attempts: 3,
        },
    }
}

fn ensure_contains(actual: &str, expected: &str) -> TestResult {
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected text to contain '{expected}'").into())
}

fn ensure_not_contains(actual: &str, unexpected: &str) -> TestResult {
    if !actual.contains(unexpected) {
        return Ok(());
    }
    Err(format!("expected text not to contain '{unexpected}'").into())
}

fn ensure_bool(actual: bool, message: &str) -> TestResult {
    if actual {
        return Ok(());
    }
    Err(message.to_owned().into())
}

fn ensure_u64(actual: u64, expected: u64) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}

fn ensure_optional_u64(actual: Option<u64>, expected: Option<u64>) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
