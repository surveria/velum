use std::{fs, path::Path};

use anyhow::Context as _;
use tabled::{Table, Tabled};

use crate::{
    RUNNER_NAME,
    bench_engines::{BenchEngine, RsqjsEngine, make_reference},
    bench_measure::{self, MeasureConfig, MeasureStats, format_duration, ratio_values},
    fenced_table, report_metadata, report_text, timing,
};

#[path = "jetstream_cases.rs"]
mod jetstream_cases;

pub const BUDGET_LABEL: &str = "1.00x";

const BUDGET_NUMERATOR: u128 = 100;
const BUDGET_DENOMINATOR: u128 = 100;
const STATUS_WITHIN_BUDGET: &str = "✅ within budget";
const STATUS_TRACKED_EXCEPTION: &str = "🟡 tracked exception";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_SKIPPED: &str = "🟡 skipped";
const STATUS_INVALID_BENCHMARK: &str = "❌ invalid benchmark";
const LATENCY_WITHIN: &str = "✅ <= 1.00x";
const LATENCY_OVER: &str = "🟡 > 1.00x";
const LATENCY_NOT_AVAILABLE: &str = "🟡 unavailable";
const LATENCY_INVALID: &str = "❌ invalid";
const QUALITY_VALID: &str = "✅ valid";
const QUALITY_INVALID: &str = "❌ invalid";
const NOT_MEASURED: &str = "-";
const DETAIL_COMPLETED: &str = "JetStream shell workload completed";
const DETAIL_LATENCY_EXCEPTION: &str = "latency budget exception tracked";
const DETAIL_QUALITY_GATE: &str = "measurement quality gate failed";
const DETAIL_REFERENCE_COMPLETED: &str = "QuickJS reference completed";
const REFERENCE_NOT_CONFIGURED: &str = "🟡 not configured";
const REFERENCE_NOT_AVAILABLE: &str = "🟡 not available";
const SHELL_PRELUDE: &str = r#"
var __rsqjsJetStreamNow = 0;
var performance = {
    now: function() { __rsqjsJetStreamNow += 1; return __rsqjsJetStreamNow; },
    mark: function() {},
    measure: function() {}
};
var console = {
    log: function() {},
    warn: function() {},
    error: function() {},
    assert: function(condition, message) {
        if (!condition)
            throw new Error(message || "console.assert failed");
    }
};
var isInBrowser = false;
// Keep JetStream feature detection on the unsupported typed-array path until
// the engine implements the broader ArrayBufferView surface these workloads use.
var ArrayBuffer = undefined;
var Uint8Array = undefined;
"#;
const SYNC_HARNESS: &str = r#"
var __rsqjsJetStreamBenchmark = new Benchmark();
var __rsqjsJetStreamResult = __rsqjsJetStreamBenchmark.runIteration();
if (__rsqjsJetStreamResult && typeof __rsqjsJetStreamResult.then === "function") {
    throw new Error("async JetStream workloads are not supported by the synchronous harness");
}
"#;

#[derive(Debug)]
pub struct JetStreamReport {
    pub rows: Vec<JetStreamRow>,
    pub measured: usize,
    pub failed: usize,
    pub invalid: usize,
    pub skipped: usize,
    pub over_latency_budget: usize,
    pub elapsed: std::time::Duration,
}

#[derive(Debug, Tabled)]
pub struct JetStreamRow {
    pub(crate) benchmark: String,
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) case_elapsed: String,
    pub(crate) rsqjs_measure: String,
    pub(crate) quickjs_measure: String,
    rsqjs_time: String,
    quickjs_time: String,
    latency_ratio: String,
    latency_budget: String,
    rsqjs_cv: String,
    quickjs_cv: String,
    quality: String,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Copy)]
struct JetStreamCase {
    id: &'static str,
    files: &'static [&'static str],
    mode: JetStreamMode,
}

impl JetStreamCase {
    const fn timed(id: &'static str, files: &'static [&'static str]) -> Self {
        Self {
            id,
            files,
            mode: JetStreamMode::Timed,
        }
    }

    const fn skipped(id: &'static str, reason: &'static str) -> Self {
        Self {
            id,
            files: &[],
            mode: JetStreamMode::Skipped(reason),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum JetStreamMode {
    Timed,
    Skipped(&'static str),
}

#[derive(Debug, Clone, Copy, Default)]
struct JetStreamCounts {
    measured: usize,
    failed: usize,
    invalid: usize,
    skipped: usize,
    over_latency_budget: usize,
}

#[derive(Debug)]
struct JetStreamOutcome {
    row: JetStreamRow,
    counts: JetStreamCounts,
}

#[derive(Debug)]
enum ReferenceMeasurement {
    NotConfigured,
    Measured(timing::Timed<MeasureStats>),
    Failed(timing::Timed<String>),
}

#[derive(Debug, Clone, Copy)]
struct BudgetCheck {
    label: &'static str,
    over_budget: bool,
}

#[must_use]
pub fn run() -> JetStreamReport {
    let timer = timing::RunTimer::start();
    let config = MeasureConfig::in_process_from_env();
    let reference = make_reference();
    let mut report = JetStreamReport {
        rows: Vec::new(),
        measured: 0,
        failed: 0,
        invalid: 0,
        skipped: 0,
        over_latency_budget: 0,
        elapsed: std::time::Duration::ZERO,
    };
    for case in jetstream_cases::cases() {
        let outcome = run_case(case, config, reference.as_deref());
        report.measured = report.measured.saturating_add(outcome.counts.measured);
        report.failed = report.failed.saturating_add(outcome.counts.failed);
        report.invalid = report.invalid.saturating_add(outcome.counts.invalid);
        report.skipped = report.skipped.saturating_add(outcome.counts.skipped);
        report.over_latency_budget = report
            .over_latency_budget
            .saturating_add(outcome.counts.over_latency_budget);
        report.rows.push(outcome.row);
    }
    report.elapsed = timer.elapsed();
    report
}

pub fn render_section(report: &JetStreamReport) -> Vec<String> {
    vec![
        "## JetStream Shell Benchmarks".to_owned(),
        String::new(),
        summary(report),
        String::new(),
        fenced_table(&Table::new(&report.rows)),
        String::new(),
    ]
}

pub fn write_report(
    path: &Path,
    metadata: &report_metadata::RunMetadata,
    report: &JetStreamReport,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create JetStream report directory '{}'",
                parent.display()
            )
        })?;
    }
    fs::write(path, render_markdown(metadata, report))
        .with_context(|| format!("failed to write JetStream report '{}'", path.display()))?;
    println!("JetStream shell benchmark report: {}", path.display());
    Ok(())
}

fn render_markdown(metadata: &report_metadata::RunMetadata, report: &JetStreamReport) -> String {
    let mut sections = vec![
        "# rs-quickjs JetStream Shell Benchmark Report".to_owned(),
        String::new(),
        format!("Generated by {RUNNER_NAME}."),
        String::new(),
    ];
    sections.extend(report_metadata::render_section(metadata));
    sections.extend(render_section(report));
    sections.join("\n")
}

fn summary(report: &JetStreamReport) -> String {
    format!(
        "- Measured: {}\n- Failed candidates: {}\n- Invalid measurements: {}\n- Skipped: {}\n- Over latency budget ({}): {}\n- Elapsed: {}",
        report.measured,
        report.failed,
        report.invalid,
        report.skipped,
        BUDGET_LABEL,
        report.over_latency_budget,
        timing::format_duration(report.elapsed),
    )
}

fn run_case(
    case: &JetStreamCase,
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
) -> JetStreamOutcome {
    match case.mode {
        JetStreamMode::Skipped(reason) => skipped_outcome(case, reason),
        JetStreamMode::Timed => run_timed_case(case, config, reference),
    }
}

fn run_timed_case(
    case: &JetStreamCase,
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
) -> JetStreamOutcome {
    let case_timer = timing::RunTimer::start();
    let source = match benchmark_source(case.files) {
        Ok(source) => source,
        Err(error) => {
            return failed_outcome(
                case,
                timing::format_duration(case_timer.elapsed()),
                &error.to_string(),
            );
        }
    };
    let ours = timing::timed(|| bench_measure::measure(config, || RsqjsEngine.eval(&source)));
    let reference = measure_reference(config, reference, &source);
    let case_elapsed = timing::format_duration(case_timer.elapsed());
    match ours.value {
        Ok(stats) => measured_with_reference_result(
            case,
            timing::Timed {
                value: stats,
                elapsed: ours.elapsed,
            },
            reference,
            case_elapsed,
        ),
        Err(error) => failed_with_reference(
            case,
            &error.to_string(),
            timing::format_duration(ours.elapsed),
            reference,
            case_elapsed,
        ),
    }
}

fn measure_reference(
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
    source: &str,
) -> ReferenceMeasurement {
    let Some(reference) = reference else {
        return ReferenceMeasurement::NotConfigured;
    };
    let measured = timing::timed(|| bench_measure::measure(config, || reference.eval(source)));
    match measured.value {
        Ok(stats) => ReferenceMeasurement::Measured(timing::Timed {
            value: stats,
            elapsed: measured.elapsed,
        }),
        Err(error) => ReferenceMeasurement::Failed(timing::Timed {
            value: format!("{}: {error}", reference.label()),
            elapsed: measured.elapsed,
        }),
    }
}

fn measured_with_reference_result(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    reference: ReferenceMeasurement,
    case_elapsed: String,
) -> JetStreamOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => {
            measured_with_reference(case, ours, reference, case_elapsed)
        }
        ReferenceMeasurement::Failed(note) => {
            reference_unavailable(case, ours, &note, case_elapsed)
        }
        ReferenceMeasurement::NotConfigured => measured_without_reference(case, ours, case_elapsed),
    }
}

fn failed_with_reference(
    case: &JetStreamCase,
    detail: &str,
    rsqjs_measure: String,
    reference: ReferenceMeasurement,
    case_elapsed: String,
) -> JetStreamOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns::failed_with_reference(
                case_elapsed,
                rsqjs_measure,
                reference.elapsed,
            ),
            timing::ReferenceColumns::measured(
                format_duration(reference.value.median()),
                reference.value.cv_percent_text(),
            ),
            reference_quality(reference.value),
            &detail_with_reference_quality(detail, reference.value),
        ),
        ReferenceMeasurement::Failed(note) => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns::failed_with_reference(
                case_elapsed,
                rsqjs_measure,
                note.elapsed,
            ),
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_AVAILABLE),
            NOT_MEASURED.to_owned(),
            &format!("{detail}; reference error: {}", note.value),
        ),
        ReferenceMeasurement::NotConfigured => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns {
                case_elapsed,
                rsqjs_measure,
                quickjs_measure: NOT_MEASURED.to_owned(),
            },
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_CONFIGURED),
            NOT_MEASURED.to_owned(),
            detail,
        ),
    }
}

fn failed_with_reference_measurement(
    case: &JetStreamCase,
    measurements: timing::MeasurementColumns,
    quickjs: timing::ReferenceColumns,
    quality: String,
    detail: &str,
) -> JetStreamOutcome {
    JetStreamOutcome {
        row: failed_row(case, measurements, quickjs, quality, detail),
        counts: JetStreamCounts {
            failed: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn reference_quality(reference: MeasureStats) -> String {
    if reference.quality().is_valid() {
        return QUALITY_VALID.to_owned();
    }
    QUALITY_INVALID.to_owned()
}

fn detail_with_reference_quality(detail: &str, reference: MeasureStats) -> String {
    let Some(quality) = reference_quality_failure_detail(reference) else {
        return format!("{detail}; {DETAIL_REFERENCE_COMPLETED}");
    };
    format!("{detail}; {quality}")
}

fn reference_quality_failure_detail(reference: MeasureStats) -> Option<String> {
    let mut reasons = Vec::new();
    collect_quality_reasons(&mut reasons, "quickjs", reference);
    if reasons.is_empty() {
        return None;
    }
    Some(format!("{DETAIL_QUALITY_GATE}: {}", reasons.join("; ")))
}

fn benchmark_source(files: &[&str]) -> anyhow::Result<String> {
    let mut script = String::new();
    script.push_str(SHELL_PRELUDE);
    script.push('\n');
    for file in files {
        let source = fs::read_to_string(file)
            .with_context(|| format!("failed to read JetStream source '{file}'"))?;
        script.push_str("// JetStream source: ");
        script.push_str(file);
        script.push('\n');
        script.push_str(&source);
        script.push('\n');
    }
    script.push_str(SYNC_HARNESS);
    Ok(script)
}

fn measured_with_reference(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    reference: timing::Timed<MeasureStats>,
    case_elapsed: String,
) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, Some(reference.value)) {
        let measurements =
            timing::MeasurementColumns::measured(case_elapsed, ours.elapsed, reference.elapsed);
        let quickjs = timing::ReferenceColumns::measured(
            format_duration(reference.value.median()),
            reference.value.cv_percent_text(),
        );
        return invalid_measurement_outcome(case, ours, measurements, quickjs, &detail, false);
    }
    let budget = budget_check(
        ours.value.median().as_nanos(),
        reference.value.median().as_nanos(),
    );
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: jetstream_status(budget.over_budget).to_owned(),
            source: case.source_label(),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: timing::format_duration(reference.elapsed),
            rsqjs_time: format_duration(ours.value.median()),
            quickjs_time: format_duration(reference.value.median()),
            latency_ratio: ratio_values(
                ours.value.median().as_nanos(),
                reference.value.median().as_nanos(),
            ),
            latency_budget: budget.label.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: reference.value.cv_percent_text(),
            quality: QUALITY_VALID.to_owned(),
            detail: jetstream_detail(&detail_text(budget.over_budget)),
        },
        counts: JetStreamCounts {
            measured: 1,
            over_latency_budget: count_if(budget.over_budget),
            ..JetStreamCounts::default()
        },
    }
}

fn measured_without_reference(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    case_elapsed: String,
) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, None) {
        let measurements =
            timing::MeasurementColumns::without_reference(case_elapsed, ours.elapsed);
        return invalid_measurement_outcome(
            case,
            ours,
            measurements,
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_CONFIGURED),
            &detail,
            true,
        );
    }
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: "✅ measured".to_owned(),
            source: case.source_label(),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: NOT_MEASURED.to_owned(),
            rsqjs_time: format_duration(ours.value.median()),
            quickjs_time: REFERENCE_NOT_CONFIGURED.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: "🟡 no reference".to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: jetstream_detail(DETAIL_COMPLETED),
        },
        counts: JetStreamCounts {
            measured: 1,
            skipped: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn reference_unavailable(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    note: &timing::Timed<String>,
    case_elapsed: String,
) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, None) {
        let measurements =
            timing::MeasurementColumns::measured(case_elapsed, ours.elapsed, note.elapsed);
        return invalid_measurement_outcome(
            case,
            ours,
            measurements,
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_AVAILABLE),
            &format!("{detail}; reference error: {}", note.value),
            true,
        );
    }
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: "✅ measured".to_owned(),
            source: case.source_label(),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: timing::format_duration(note.elapsed),
            rsqjs_time: format_duration(ours.value.median()),
            quickjs_time: REFERENCE_NOT_AVAILABLE.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: LATENCY_NOT_AVAILABLE.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: jetstream_detail(&format!(
                "{DETAIL_COMPLETED}; reference error: {}",
                note.value
            )),
        },
        counts: JetStreamCounts {
            measured: 1,
            skipped: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn invalid_measurement_outcome(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    measurements: timing::MeasurementColumns,
    quickjs: timing::ReferenceColumns,
    detail: &str,
    skipped_reference: bool,
) -> JetStreamOutcome {
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: STATUS_INVALID_BENCHMARK.to_owned(),
            source: case.source_label(),
            case_elapsed: measurements.case_elapsed,
            rsqjs_measure: measurements.rsqjs_measure,
            quickjs_measure: measurements.quickjs_measure,
            rsqjs_time: format_duration(ours.value.median()),
            quickjs_time: quickjs.eval,
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: LATENCY_INVALID.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: quickjs.cv,
            quality: QUALITY_INVALID.to_owned(),
            detail: jetstream_detail(detail),
        },
        counts: JetStreamCounts {
            measured: 1,
            failed: 1,
            invalid: 1,
            skipped: count_if(skipped_reference),
            ..JetStreamCounts::default()
        },
    }
}

fn failed_outcome(case: &JetStreamCase, case_elapsed: String, detail: &str) -> JetStreamOutcome {
    JetStreamOutcome {
        row: failed_row(
            case,
            timing::MeasurementColumns::not_measured(case_elapsed),
            timing::ReferenceColumns::not_measured(NOT_MEASURED),
            NOT_MEASURED.to_owned(),
            detail,
        ),
        counts: JetStreamCounts {
            failed: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn failed_row(
    case: &JetStreamCase,
    measurements: timing::MeasurementColumns,
    quickjs: timing::ReferenceColumns,
    quality: String,
    detail: &str,
) -> JetStreamRow {
    JetStreamRow {
        benchmark: case.id.to_owned(),
        status: STATUS_FAILED.to_owned(),
        source: case.source_label(),
        case_elapsed: measurements.case_elapsed,
        rsqjs_measure: measurements.rsqjs_measure,
        quickjs_measure: measurements.quickjs_measure,
        rsqjs_time: NOT_MEASURED.to_owned(),
        quickjs_time: quickjs.eval,
        latency_ratio: NOT_MEASURED.to_owned(),
        latency_budget: NOT_MEASURED.to_owned(),
        rsqjs_cv: NOT_MEASURED.to_owned(),
        quickjs_cv: quickjs.cv,
        quality,
        detail: jetstream_detail(detail),
    }
}

fn skipped_outcome(case: &JetStreamCase, reason: &str) -> JetStreamOutcome {
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: STATUS_SKIPPED.to_owned(),
            source: case.source_label(),
            case_elapsed: NOT_MEASURED.to_owned(),
            rsqjs_measure: NOT_MEASURED.to_owned(),
            quickjs_measure: NOT_MEASURED.to_owned(),
            rsqjs_time: NOT_MEASURED.to_owned(),
            quickjs_time: NOT_MEASURED.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: NOT_MEASURED.to_owned(),
            rsqjs_cv: NOT_MEASURED.to_owned(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: NOT_MEASURED.to_owned(),
            detail: jetstream_detail(reason),
        },
        counts: JetStreamCounts {
            skipped: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn jetstream_detail(detail: &str) -> String {
    report_text::table_detail(detail)
}

fn quality_failure_detail(ours: MeasureStats, reference: Option<MeasureStats>) -> Option<String> {
    if ours.quality().is_valid() && reference.is_none_or(|reference| reference.quality().is_valid())
    {
        return None;
    }
    let mut reasons = Vec::new();
    collect_quality_reasons(&mut reasons, "rsqjs", ours);
    if let Some(reference) = reference {
        collect_quality_reasons(&mut reasons, "quickjs", reference);
    }
    if reasons.is_empty() {
        return None;
    }
    Some(format!("{DETAIL_QUALITY_GATE}: {}", reasons.join("; ")))
}

fn collect_quality_reasons(reasons: &mut Vec<String>, label: &str, stats: MeasureStats) {
    let quality = stats.quality();
    if quality.low_signal() {
        reasons.push(format!(
            "{label} median {} below minimum {}",
            format_duration(stats.median()),
            format_duration(quality.min_op_time())
        ));
    }
    if quality.high_variance() {
        reasons.push(format!(
            "{label} CV {} exceeds maximum {}",
            stats.cv_percent_text(),
            quality.max_cv_percent_text()
        ));
    }
    if quality.iteration_cap_reached() {
        reasons.push(format!(
            "{label} calibration reached iteration cap; median sample {}",
            format_duration(stats.median_sample())
        ));
    }
}

fn detail_text(over_latency_budget: bool) -> String {
    if over_latency_budget {
        return format!("{DETAIL_COMPLETED}; {DETAIL_LATENCY_EXCEPTION}");
    }
    DETAIL_COMPLETED.to_owned()
}

const fn budget_check(ours: u128, reference: u128) -> BudgetCheck {
    if reference == 0 {
        return BudgetCheck {
            label: LATENCY_NOT_AVAILABLE,
            over_budget: false,
        };
    }
    let over_budget =
        ours.saturating_mul(BUDGET_DENOMINATOR) > reference.saturating_mul(BUDGET_NUMERATOR);
    BudgetCheck {
        label: if over_budget {
            LATENCY_OVER
        } else {
            LATENCY_WITHIN
        },
        over_budget,
    }
}

const fn jetstream_status(over_latency_budget: bool) -> &'static str {
    if over_latency_budget {
        return STATUS_TRACKED_EXCEPTION;
    }
    STATUS_WITHIN_BUDGET
}

const fn count_if(condition: bool) -> usize {
    if condition { 1 } else { 0 }
}

impl JetStreamCase {
    fn source_label(self) -> String {
        match self.files {
            [] => NOT_MEASURED.to_owned(),
            [file] => (*file).to_owned(),
            [first, ..] => format!("{} (+{} more)", first, self.files.len().saturating_sub(1)),
        }
    }
}

#[cfg(test)]
#[path = "jetstream_tests.rs"]
mod tests;
