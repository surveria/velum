use std::time::Duration;

use crate::{
    CaseRow, CorpusReport, CorpusStats, FeatureAreaRow, FullReport, STATUS_PASSED, benchmarks,
    jetstream, report_metadata,
    report_rendering::{render_report, render_timing_tsv},
    report_schema::{
        BenchmarkConfiguration, BenchmarkStatus, CaseCounts, CaseDetailCoverage,
        DetailCompleteness, DetailLevel, EnvironmentInfo, MAX_FAILURE_DIAGNOSTICS, ReportDocument,
        ReportSummary, RunConfiguration, SCHEMA_VERSION, SuiteStatus,
    },
    report_schema_io::MAX_CANONICAL_YAML_LINES,
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
    ensure_contains(&yaml, "mode: prepared_execution")?;
    ensure_contains(&yaml, "reference_source: quickjs_baseline")?;
    ensure_contains(&yaml, "duration_ns: 10000")?;
    ensure_contains(&yaml, "bits: 4631107791820423168")?;

    let decoded: ReportDocument = serde_yaml_ng::from_str(&yaml)?;
    decoded.validate()?;
    ensure_bool(decoded == report, "full YAML round-trip changed the report")
}

#[test]
fn compact_jetstream_yaml_keeps_every_official_row_within_the_line_contract() -> TestResult {
    const JETSTREAM_LINE_HEADROOM_TARGET: usize = 950;
    let source = jetstream::worst_case_report_fixture();
    let expected_rows = source.rows.len();
    let report = ReportDocument::from_jetstream_run(
        &source,
        report_metadata::RunMetadata::default(),
        sample_environment(),
        sample_jetstream_configuration(),
    )?;
    let markdown = render_report(&report);
    ensure_contains(&markdown, "case_elapsed")?;
    ensure_contains(&markdown, "velum_measure")?;
    ensure_contains(&markdown, "quickjs_measure")?;
    ensure_contains(&markdown, "latency_budget")?;
    ensure_contains(&markdown, "quality")?;
    ensure_contains(&markdown, "quickjs_baseline")?;
    let tsv = render_timing_tsv(&report);
    let jetstream_line = tsv
        .lines()
        .find(|line| line.starts_with("jetstream\tJetStream Shell Benchmarks\tAir\t"))
        .ok_or("expected JetStream TSV row for Air")?;
    let fields = jetstream_line.split('\t').collect::<Vec<_>>();
    ensure_bool(
        fields.get(5) == Some(&"")
            && fields.get(6) == Some(&"2.00 ms")
            && fields.get(7) == Some(&"1.50 ms")
            && fields.get(8) == Some(&"1.50 ms"),
        "JetStream TSV lifecycle columns are misaligned",
    )?;
    let bounded = report.bounded_component()?;
    let bounded_markdown = render_report(&bounded);
    ensure_contains(&bounded_markdown, "case_elapsed")?;
    ensure_contains(&bounded_markdown, "2.00 ms")?;
    ensure_contains(&bounded_markdown, "✅ valid")?;
    ensure_bool(
        bounded.jetstream.rows.len() == expected_rows,
        "bounded JetStream YAML omitted official rows",
    )?;
    let component_yaml = serde_yaml_ng::to_string(&bounded)?;
    let summary_yaml = serde_yaml_ng::to_string(&bounded.summary())?;
    let component_lines = component_yaml.lines().count();
    let summary_lines = summary_yaml.lines().count();
    ensure_bool(
        component_lines <= JETSTREAM_LINE_HEADROOM_TARGET
            && summary_lines <= JETSTREAM_LINE_HEADROOM_TARGET,
        &format!(
            "worst-case JetStream YAML exceeded {JETSTREAM_LINE_HEADROOM_TARGET}-line headroom target (hard maximum {MAX_CANONICAL_YAML_LINES}): component={component_lines}, summary={summary_lines}"
        ),
    )?;
    ensure_contains(&summary_yaml, "reference_source: quickjs_baseline")?;
    ensure_contains(&summary_yaml, "total: 86")
}

#[test]
fn current_schema_reads_the_tracked_pre_compact_v1_summary() -> TestResult {
    let yaml = include_str!("../../reports/test-runs/velum-test-report-20260709T230501Z.yaml");
    let report: ReportSummary = serde_yaml_ng::from_str(yaml)?;
    report.validate()?;
    ensure_bool(
        report.jetstream.rows.is_empty(),
        "tracked compatibility fixture unexpectedly contains JetStream rows",
    )
}

#[test]
fn jetstream_validation_rejects_aggregate_drift_and_quickjs_in_strict_read() -> TestResult {
    let mut aggregate_drift = sample_jetstream_document()?;
    aggregate_drift.jetstream.counts.measured =
        aggregate_drift.jetstream.counts.measured.saturating_sub(1);
    ensure_bool(
        aggregate_drift.validate().is_err(),
        "JetStream aggregate drift was accepted",
    )?;

    let mut strict_read = sample_jetstream_document()?;
    strict_read
        .configuration
        .benchmark
        .reference_quickjs_compiled = true;
    ensure_bool(
        strict_read.validate().is_err(),
        "strict JetStream read accepted a compiled QuickJS reference",
    )
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
    ensure_optional_u64(benchmark.latency_ratio_centi_units, Some(125))?;
    let Some(methodology) = &benchmark.methodology else {
        return Err("expected benchmark methodology".into());
    };
    let Some(lifecycle) = &methodology.lifecycle else {
        return Err("expected benchmark lifecycle".into());
    };
    ensure_optional_u64(lifecycle.load.duration_ns, Some(10_000))
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
fn schema_validation_rejects_overflowing_suite_counts() -> TestResult {
    let report = sample_document()?;
    let mut summary = report.summary();
    let Some(suite) = summary.suites.first_mut() else {
        return Err("expected one suite summary".into());
    };
    suite.counts.passed = u64::MAX;
    suite.counts.failed = 1;
    suite.counts.executed = u64::MAX;
    suite.counts.total = u64::MAX;
    suite.case_details.recorded_rows = 0;
    suite.case_details.omitted_rows = u64::MAX;
    suite.case_details.completeness = DetailCompleteness::Partial;
    ensure_bool(
        summary.validate().is_err(),
        "overflowing suite counts must be rejected",
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
fn partial_case_details_reject_contradictory_statuses_and_duplicate_ids() -> TestResult {
    let mut report = sample_document()?;
    let Some(suite) = report.suites.first_mut() else {
        return Err("expected one suite".into());
    };
    let Some(case) = suite.cases.first_mut() else {
        return Err("expected one case".into());
    };
    case.status = crate::report_schema::CaseStatus::Failed;
    suite.summary.counts = CaseCounts {
        total: 100,
        executed: 100,
        passed: 100,
        failed: 0,
        skipped: 0,
    };
    suite.summary.case_details = CaseDetailCoverage {
        completeness: DetailCompleteness::Partial,
        recorded_rows: 1,
        omitted_rows: 99,
    };
    ensure_bool(
        report.validate().is_err(),
        "partial details accepted a failed row above the summary count",
    )?;

    let mut report = sample_document()?;
    let Some(suite) = report.suites.first_mut() else {
        return Err("expected one suite".into());
    };
    let Some(case) = suite.cases.first().cloned() else {
        return Err("expected one case".into());
    };
    suite.cases.push(case);
    suite.summary.counts = CaseCounts {
        total: 2,
        executed: 2,
        passed: 2,
        failed: 0,
        skipped: 0,
    };
    suite.summary.case_details = CaseDetailCoverage {
        completeness: DetailCompleteness::Complete,
        recorded_rows: 2,
        omitted_rows: 0,
    };
    ensure_bool(
        report.validate().is_err(),
        "duplicate case ids must be rejected",
    )
}

#[test]
fn schema_validation_rejects_inconsistent_benchmark_counts() -> TestResult {
    let report = sample_document()?;
    let mut summary = report.summary();
    summary.benchmarks.counts.measured = 0;
    if summary.validate().is_err() {
        return Ok(());
    }
    Err("inconsistent benchmark counts must be rejected".into())
}

#[test]
fn schema_validation_rejects_inconsistent_measurement_and_ratio_metadata() -> TestResult {
    let mut report = sample_document()?;
    let Some(row) = report.benchmarks.rows.first_mut() else {
        return Err("expected one benchmark".into());
    };
    row.engine.availability = crate::report_schema::MeasurementAvailability::NotMeasured;
    ensure_bool(
        report.validate().is_err(),
        "measurement availability accepted typed duration fields",
    )?;

    let mut report = sample_document()?;
    let Some(row) = report.benchmarks.rows.first_mut() else {
        return Err("expected one benchmark".into());
    };
    row.reference.availability = crate::report_schema::MeasurementAvailability::NotConfigured;
    row.reference.wall_duration_ns = None;
    row.reference.median_duration_ns = None;
    row.reference.coefficient_variation_permille = None;
    let Some(methodology) = row.methodology.as_mut() else {
        return Err("expected benchmark methodology".into());
    };
    methodology.reference_source =
        Some(crate::report_benchmark_methodology::ReferenceSource::NotConfigured);
    ensure_bool(
        report.validate().is_err(),
        "ratio without both measurements must be rejected",
    )
}

#[test]
fn typed_count_contribution_distinguishes_parity_and_quality_failures() -> TestResult {
    validate_parity_failure_contribution(true)?;
    validate_parity_failure_contribution(false)
}

#[test]
fn bounded_diagnostics_are_deterministic_complete_and_size_limited() -> TestResult {
    let first = diagnostic_document()?;
    let second = diagnostic_document()?;
    let Some(first_suite) = first.suites.first() else {
        return Err("expected diagnostic suite".into());
    };
    let Some(first_diagnostics) = &first_suite.summary.failure_diagnostics else {
        return Err("expected failure diagnostics".into());
    };
    let Some(second_diagnostics) = second
        .suites
        .first()
        .and_then(|suite| suite.summary.failure_diagnostics.as_ref())
    else {
        return Err("expected second failure diagnostics".into());
    };
    ensure_bool(
        first_diagnostics == second_diagnostics,
        "failure diagnostics are not deterministic",
    )?;
    ensure_u64(first_diagnostics.total_failed, 40)?;
    ensure_u64(first_diagnostics.total_groups, 40)?;
    ensure_u64(first_diagnostics.omitted_groups, 10)?;
    ensure_bool(
        first_diagnostics.groups.len() == MAX_FAILURE_DIAGNOSTICS,
        "failure diagnostics exceeded or missed the global group cap",
    )?;
    ensure_bool(
        first_diagnostics.categories.len() == 4,
        "expected four exact failure categories",
    )?;
    let category_total = first_diagnostics
        .categories
        .iter()
        .try_fold(0u64, |total, category| total.checked_add(category.failed))
        .ok_or_else(|| anyhow::anyhow!("failure category total overflowed"))?;
    ensure_u64(category_total, 40)?;

    let bounded = first.bounded_component()?;
    let component_yaml = serde_yaml_ng::to_string(&bounded)?;
    let summary_yaml = serde_yaml_ng::to_string(&bounded.summary())?;
    ensure_bool(
        component_yaml.lines().count() <= MAX_CANONICAL_YAML_LINES,
        "bounded component exceeded the YAML line contract",
    )?;
    ensure_bool(
        summary_yaml.lines().count() <= MAX_CANONICAL_YAML_LINES,
        "bounded summary exceeded the YAML line contract",
    )
}

pub fn diagnostic_document() -> Result<ReportDocument, anyhow::Error> {
    let mut rows = (0..40)
        .map(|index| CaseRow {
            case: format!("failed-{index:02}"),
            status: crate::STATUS_FAILED.to_owned(),
            source: format!("test262:test/built-ins/Feature{index:02}/failed.js"),
            detail: diagnostic_detail(index),
            elapsed: Duration::from_micros(1),
        })
        .collect::<Vec<_>>();
    rows.extend((0..40).map(|index| CaseRow {
        case: format!("passed-{index:02}"),
        status: STATUS_PASSED.to_owned(),
        source: format!("test262:test/built-ins/Feature{index:02}/passed.js"),
        detail: "matched expected behavior".to_owned(),
        elapsed: Duration::from_micros(1),
    }));
    let report = FullReport {
        metadata: report_metadata::RunMetadata::default(),
        corpora: vec![CorpusReport {
            name: "Diagnostic corpus",
            required: true,
            stats: CorpusStats {
                total: rows.len(),
                passed: 40,
                failed: 40,
                skipped: 0,
            },
            rows,
            skip_reasons: Vec::new(),
            feature_areas: diagnostic_feature_areas(),
            elapsed: Duration::from_millis(1),
        }],
        benchmarks: benchmarks::BenchmarkReport::not_run(),
        jetstream: jetstream::JetStreamReport::not_run(),
        elapsed: Duration::from_millis(1),
    };
    ReportDocument::from_run(report, sample_environment(), sample_configuration())
}

fn diagnostic_feature_areas() -> Vec<FeatureAreaRow> {
    (0..33)
        .map(|index| FeatureAreaRow {
            feature_area: if index == 32 {
                "other".to_owned()
            } else {
                format!("built-ins/Feature{index:02}")
            },
            total: if index == 32 { 16 } else { 2 },
            executed: if index == 32 { 16 } else { 2 },
            passed: if index == 32 { "8 passed" } else { "1 passed" }.to_owned(),
            failed: if index == 32 { "8 failed" } else { "1 failed" }.to_owned(),
            skipped: "0 skipped".to_owned(),
            pass_rate: "100.0%".to_owned(),
            manifest_enabled: 1,
            top_skip_reason: "none".to_owned(),
        })
        .collect()
}

fn diagnostic_detail(index: usize) -> String {
    match index % 4 {
        0 => format!("parser error: expected token-{index:02}"),
        1 => format!("runtime error: ReferenceError: 'binding{index:02}' is not defined"),
        2 => format!("lexer error: unexpected character code-{index:02}"),
        _ => format!("metadata error: unsupported negative phase '{index:02}'"),
    }
}

fn validate_parity_failure_contribution(counted_invalid: bool) -> TestResult {
    let mut report = sample_document()?;
    let Some(row) = report.benchmarks.rows.first_mut() else {
        return Err("expected one benchmark".into());
    };
    row.status = crate::report_schema::BenchmarkStatus::Failed;
    row.quality = crate::report_schema::QualityStatus::Invalid;
    row.latency_ratio_centi_units = None;
    row.latency_budget = crate::report_schema::BudgetStatus::Invalid;
    let Some(contribution) = row.count_contribution.as_mut() else {
        return Err("expected benchmark count contribution".into());
    };
    contribution.failed = crate::report_schema::BenchmarkContributionFlag::Counted;
    contribution.invalid = if counted_invalid {
        crate::report_schema::BenchmarkContributionFlag::Counted
    } else {
        crate::report_schema::BenchmarkContributionFlag::NotCounted
    };
    contribution.over_latency_budget = crate::report_schema::BenchmarkContributionFlag::NotCounted;
    report.benchmarks.counts.failed = 1;
    report.benchmarks.counts.invalid = u64::from(counted_invalid);
    report.benchmarks.counts.over_latency_budget = 0;
    report.validate()?;
    Ok(())
}

#[test]
fn structured_report_renders_compatible_markdown_and_tsv() -> TestResult {
    let report = sample_document()?;
    let markdown = render_report(&report);
    ensure_contains(&markdown, "# Velum Test Report")?;
    ensure_contains(&markdown, "## Unit corpus")?;
    ensure_contains(&markdown, "array-index")?;
    ensure_contains(&markdown, "1.25x")?;
    ensure_contains(&markdown, "prepared_execution")?;
    ensure_contains(&markdown, "quickjs_baseline")?;

    let tsv = render_timing_tsv(&report);
    ensure_contains(&tsv, "kind\tphase\tcase\tstatus")?;
    ensure_contains(
        &tsv,
        "test\tUnit corpus\tcase-1\t✅ passed\ttests/example.js\t\t1.250000",
    )?;
    ensure_contains(
        &tsv,
        "benchmark\tBenchmarks\tarray-index\t🟡 tracked exception",
    )?;
    ensure_contains(&tsv, "prepared_execution\tload=10.00 us")
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
                velum_measure: "2.00 ms".to_owned(),
                quickjs_measure: "3.00 ms".to_owned(),
                velum_eval: "1.25 ms".to_owned(),
                quickjs_eval: "1.00 ms".to_owned(),
                latency_ratio: "1.25x".to_owned(),
                latency_budget: "🟡 > 1.00x".to_owned(),
                memory_ratio: "-".to_owned(),
                velum_cv: "2.0%".to_owned(),
                quickjs_cv: "1.0%".to_owned(),
                quality: "✅ valid".to_owned(),
                detail: "sequential benchmark completed".to_owned(),
                mode: "prepared_execution".to_owned(),
                lifecycle: "load=10.00 us;compile=20.00 us;setup=30.00 us;warmup=1.00 ms;run=5.00 ms;verify=40.00 us;teardown=50.00 us".to_owned(),
                checksum: "42".to_owned(),
                reference_source: "quickjs_baseline".to_owned(),
                methodology: crate::benchmark_protocol::BenchmarkMethodology {
                    mode: Some(crate::benchmark_protocol::BenchmarkMode::PreparedExecution),
                    lifecycle: Some(crate::benchmark_protocol::ReportedLifecycle::prepared(
                        crate::benchmark_protocol::BenchmarkLifecycle {
                            load: Duration::from_micros(10),
                            compile: Some(Duration::from_micros(20)),
                            setup: Some(Duration::from_micros(30)),
                            warmup: Duration::from_millis(1),
                            timed_run: Duration::from_millis(5),
                            verify: Some(Duration::from_micros(40)),
                            teardown: Some(Duration::from_micros(50)),
                        },
                    )),
                    checksum: Some(crate::benchmark_protocol::BenchmarkChecksum::number(42.0)),
                    reference_source: Some(
                        crate::benchmark_protocol::BenchmarkReferenceSource::QuickjsBaseline,
                    ),
                },
                count_contribution: crate::benchmark_protocol::BenchmarkCountContribution {
                    measured: crate::benchmark_protocol::BenchmarkContributionFlag::Counted,
                    in_process_measured:
                        crate::benchmark_protocol::BenchmarkContributionFlag::Counted,
                    failed: crate::benchmark_protocol::BenchmarkContributionFlag::NotCounted,
                    invalid: crate::benchmark_protocol::BenchmarkContributionFlag::NotCounted,
                    skipped_reference:
                        crate::benchmark_protocol::BenchmarkContributionFlag::NotCounted,
                    over_latency_budget:
                        crate::benchmark_protocol::BenchmarkContributionFlag::Counted,
                    over_memory_budget:
                        crate::benchmark_protocol::BenchmarkContributionFlag::NotCounted,
                },
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
            reference_missing: 0,
            elapsed: Duration::ZERO,
        },
        elapsed: Duration::from_millis(7),
    };
    ReportDocument::from_run(report, sample_environment(), sample_configuration())
}

pub fn sample_jetstream_document() -> Result<ReportDocument, anyhow::Error> {
    let report = jetstream::worst_case_report_fixture();
    ReportDocument::from_jetstream_run(
        &report,
        report_metadata::RunMetadata::default(),
        sample_environment(),
        sample_jetstream_configuration(),
    )
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
        benchmark_set: crate::report_schema::BenchmarkSet::Full,
        benchmark_filter: None,
        quickjs_baseline: crate::report_schema::QuickjsBaselineMode::Read,
        benchmark: BenchmarkConfiguration {
            reference_quickjs_compiled: true,
            warmup_duration_ns: 150_000_000,
            minimum_sample_duration_ns: 500_000_000,
            samples: 10,
            minimum_operation_duration_ns: 1_000_000,
            maximum_cv_permille: 100,
            attempts: 3,
            maximum_operation_duration_ns: 2_000_000_000,
            maximum_total_duration_ns: 3_000_000_000,
        },
        suite_max_duration_ns: None,
    }
}

fn sample_jetstream_configuration() -> RunConfiguration {
    let mut configuration = sample_configuration();
    configuration.report_mode = crate::report_schema::ReportMode::Jetstream;
    configuration.jetstream = crate::report_schema::FeatureSelection::Enabled;
    configuration.quickjs_differential = crate::report_schema::InputAvailability::NotConfigured;
    configuration.test262 = crate::report_schema::InputAvailability::NotConfigured;
    configuration.test262_mode = crate::report_schema::Test262Mode::Manifest;
    configuration.benchmark.reference_quickjs_compiled = false;
    configuration.suite_max_duration_ns = Some(120_000_000_000);
    configuration
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
