use std::{fs, path::Path};

use anyhow::Context as _;
use tabled::{Table, Tabled};

use crate::{
    RUNNER_NAME,
    bench_engines::{BenchEngine, RsqjsEngine, make_reference},
    bench_measure::{self, MeasureConfig, MeasureStats, format_duration, ratio_values},
    fenced_table, report_metadata,
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
}

#[derive(Debug, Tabled)]
pub struct JetStreamRow {
    benchmark: String,
    status: String,
    source: String,
    rsqjs_time: String,
    quickjs_time: String,
    latency_ratio: String,
    latency_budget: String,
    rsqjs_cv: String,
    quickjs_cv: String,
    quality: String,
    detail: String,
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
    Measured(MeasureStats),
    Failed(String),
}

#[derive(Debug, Clone, Copy)]
struct BudgetCheck {
    label: &'static str,
    over_budget: bool,
}

#[must_use]
pub fn run() -> JetStreamReport {
    let config = MeasureConfig::in_process_from_env();
    let reference = make_reference();
    let mut report = JetStreamReport {
        rows: Vec::new(),
        measured: 0,
        failed: 0,
        invalid: 0,
        skipped: 0,
        over_latency_budget: 0,
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
        "- Measured: {}\n- Failed candidates: {}\n- Invalid measurements: {}\n- Skipped: {}\n- Over latency budget ({}): {}",
        report.measured,
        report.failed,
        report.invalid,
        report.skipped,
        BUDGET_LABEL,
        report.over_latency_budget,
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
    let source = match benchmark_source(case.files) {
        Ok(source) => source,
        Err(error) => return failed_outcome(case, &error.to_string()),
    };
    let ours = bench_measure::measure(config, || RsqjsEngine.eval(&source));
    let reference = measure_reference(config, reference, &source);
    match ours {
        Ok(stats) => measured_with_reference_result(case, stats, reference),
        Err(error) => failed_with_reference(case, &error.to_string(), reference),
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
    match bench_measure::measure(config, || reference.eval(source)) {
        Ok(stats) => ReferenceMeasurement::Measured(stats),
        Err(error) => ReferenceMeasurement::Failed(format!("{}: {error}", reference.label())),
    }
}

fn measured_with_reference_result(
    case: &JetStreamCase,
    ours: MeasureStats,
    reference: ReferenceMeasurement,
) -> JetStreamOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => measured_with_reference(case, ours, reference),
        ReferenceMeasurement::Failed(note) => reference_unavailable(case, ours, &note),
        ReferenceMeasurement::NotConfigured => measured_without_reference(case, ours),
    }
}

fn failed_with_reference(
    case: &JetStreamCase,
    detail: &str,
    reference: ReferenceMeasurement,
) -> JetStreamOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => failed_with_reference_measurement(
            case,
            format_duration(reference.median()),
            reference.cv_percent_text(),
            reference_quality(reference),
            &detail_with_reference_quality(detail, reference),
        ),
        ReferenceMeasurement::Failed(note) => failed_with_reference_measurement(
            case,
            REFERENCE_NOT_AVAILABLE.to_owned(),
            NOT_MEASURED.to_owned(),
            NOT_MEASURED.to_owned(),
            &format!("{detail}; reference error: {note}"),
        ),
        ReferenceMeasurement::NotConfigured => failed_with_reference_measurement(
            case,
            REFERENCE_NOT_CONFIGURED.to_owned(),
            NOT_MEASURED.to_owned(),
            NOT_MEASURED.to_owned(),
            detail,
        ),
    }
}

fn failed_with_reference_measurement(
    case: &JetStreamCase,
    quickjs_time: String,
    quickjs_cv: String,
    quality: String,
    detail: &str,
) -> JetStreamOutcome {
    JetStreamOutcome {
        row: failed_row(case, quickjs_time, quickjs_cv, quality, detail),
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
    ours: MeasureStats,
    reference: MeasureStats,
) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours, Some(reference)) {
        return invalid_measurement_outcome(
            case,
            ours,
            format_duration(reference.median()),
            reference.cv_percent_text(),
            &detail,
            false,
        );
    }
    let budget = budget_check(ours.median().as_nanos(), reference.median().as_nanos());
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: jetstream_status(budget.over_budget).to_owned(),
            source: case.source_label(),
            rsqjs_time: format_duration(ours.median()),
            quickjs_time: format_duration(reference.median()),
            latency_ratio: ratio_values(ours.median().as_nanos(), reference.median().as_nanos()),
            latency_budget: budget.label.to_owned(),
            rsqjs_cv: ours.cv_percent_text(),
            quickjs_cv: reference.cv_percent_text(),
            quality: QUALITY_VALID.to_owned(),
            detail: detail_text(budget.over_budget),
        },
        counts: JetStreamCounts {
            measured: 1,
            over_latency_budget: count_if(budget.over_budget),
            ..JetStreamCounts::default()
        },
    }
}

fn measured_without_reference(case: &JetStreamCase, ours: MeasureStats) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours, None) {
        return invalid_measurement_outcome(
            case,
            ours,
            REFERENCE_NOT_CONFIGURED.to_owned(),
            NOT_MEASURED.to_owned(),
            &detail,
            true,
        );
    }
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: "✅ measured".to_owned(),
            source: case.source_label(),
            rsqjs_time: format_duration(ours.median()),
            quickjs_time: REFERENCE_NOT_CONFIGURED.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: "🟡 no reference".to_owned(),
            rsqjs_cv: ours.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: DETAIL_COMPLETED.to_owned(),
        },
        counts: JetStreamCounts {
            measured: 1,
            skipped: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn reference_unavailable(case: &JetStreamCase, ours: MeasureStats, note: &str) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours, None) {
        return invalid_measurement_outcome(
            case,
            ours,
            REFERENCE_NOT_AVAILABLE.to_owned(),
            NOT_MEASURED.to_owned(),
            &format!("{detail}; reference error: {note}"),
            true,
        );
    }
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: "✅ measured".to_owned(),
            source: case.source_label(),
            rsqjs_time: format_duration(ours.median()),
            quickjs_time: REFERENCE_NOT_AVAILABLE.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: LATENCY_NOT_AVAILABLE.to_owned(),
            rsqjs_cv: ours.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: format!("{DETAIL_COMPLETED}; reference error: {note}"),
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
    ours: MeasureStats,
    quickjs_time: String,
    quickjs_cv: String,
    detail: &str,
    skipped_reference: bool,
) -> JetStreamOutcome {
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: STATUS_INVALID_BENCHMARK.to_owned(),
            source: case.source_label(),
            rsqjs_time: format_duration(ours.median()),
            quickjs_time,
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: LATENCY_INVALID.to_owned(),
            rsqjs_cv: ours.cv_percent_text(),
            quickjs_cv,
            quality: QUALITY_INVALID.to_owned(),
            detail: detail.to_owned(),
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

fn failed_outcome(case: &JetStreamCase, detail: &str) -> JetStreamOutcome {
    JetStreamOutcome {
        row: failed_row(
            case,
            NOT_MEASURED.to_owned(),
            NOT_MEASURED.to_owned(),
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
    quickjs_time: String,
    quickjs_cv: String,
    quality: String,
    detail: &str,
) -> JetStreamRow {
    JetStreamRow {
        benchmark: case.id.to_owned(),
        status: STATUS_FAILED.to_owned(),
        source: case.source_label(),
        rsqjs_time: NOT_MEASURED.to_owned(),
        quickjs_time,
        latency_ratio: NOT_MEASURED.to_owned(),
        latency_budget: NOT_MEASURED.to_owned(),
        rsqjs_cv: NOT_MEASURED.to_owned(),
        quickjs_cv,
        quality,
        detail: detail.to_owned(),
    }
}

fn skipped_outcome(case: &JetStreamCase, reason: &str) -> JetStreamOutcome {
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: STATUS_SKIPPED.to_owned(),
            source: case.source_label(),
            rsqjs_time: NOT_MEASURED.to_owned(),
            quickjs_time: NOT_MEASURED.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: NOT_MEASURED.to_owned(),
            rsqjs_cv: NOT_MEASURED.to_owned(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: NOT_MEASURED.to_owned(),
            detail: reason.to_owned(),
        },
        counts: JetStreamCounts {
            skipped: 1,
            ..JetStreamCounts::default()
        },
    }
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
mod tests {
    use super::{LATENCY_OVER, LATENCY_WITHIN, benchmark_source, budget_check};
    use std::time::Duration;

    #[test]
    fn budget_check_treats_faster_rsquickjs_as_within_budget() -> anyhow::Result<()> {
        let check = budget_check(90, 100);
        ensure_bool(!check.over_budget, "faster rsqjs must be within budget")?;
        ensure_text(check.label, LATENCY_WITHIN)
    }

    #[test]
    fn budget_check_tracks_slower_rsquickjs_as_exception() -> anyhow::Result<()> {
        let check = budget_check(101, 100);
        ensure_bool(check.over_budget, "slower rsqjs must be tracked")?;
        ensure_text(check.label, LATENCY_OVER)
    }

    #[test]
    fn benchmark_source_appends_sync_harness() -> anyhow::Result<()> {
        let source = benchmark_source(&[])?;
        ensure_bool(
            source.contains("new Benchmark()"),
            "harness must construct benchmark",
        )?;
        ensure_bool(
            source.contains("runIteration()"),
            "harness must run iteration",
        )
    }

    #[test]
    fn failed_jetstream_candidate_preserves_quickjs_measurement() -> anyhow::Result<()> {
        let case = super::JetStreamCase::timed(
            "failed-candidate",
            &["tests/external/jetstream/simple/hash-map.js"],
        );
        let reference = sample_stats()?;
        let outcome = super::failed_with_reference(
            &case,
            "rsqjs eval failed: sample",
            super::ReferenceMeasurement::Measured(reference),
        );
        ensure_text(&outcome.row.status, super::STATUS_FAILED)?;
        ensure_text(&outcome.row.rsqjs_time, super::NOT_MEASURED)?;
        ensure_bool(
            outcome.row.quickjs_time != super::NOT_MEASURED,
            "failed row must retain QuickJS timing",
        )?;
        ensure_bool(
            outcome.row.quickjs_cv != super::NOT_MEASURED,
            "failed row must retain QuickJS variation",
        )?;
        ensure_usize(outcome.counts.failed, 1)
    }

    fn sample_stats() -> Result<crate::bench_measure::MeasureStats, anyhow::Error> {
        let config =
            super::MeasureConfig::new(Duration::from_millis(0), Duration::from_millis(1), 3)
                .with_quality(Duration::ZERO, u32::MAX);
        crate::bench_measure::measure(config, || {
            std::hint::black_box(42_u64);
            Ok::<(), anyhow::Error>(())
        })
    }

    fn ensure_text(actual: &str, expected: &str) -> anyhow::Result<()> {
        if actual == expected {
            return Ok(());
        }
        Err(anyhow::anyhow!("expected '{expected}', got '{actual}'"))
    }

    fn ensure_bool(actual: bool, message: &str) -> anyhow::Result<()> {
        if actual {
            return Ok(());
        }
        Err(anyhow::anyhow!(message.to_owned()))
    }

    fn ensure_usize(actual: usize, expected: usize) -> anyhow::Result<()> {
        if actual == expected {
            return Ok(());
        }
        Err(anyhow::anyhow!("expected {expected}, got {actual}"))
    }
}
