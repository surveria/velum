//! In-process benchmark harness.
//!
//! Both the engine under test and the optional `QuickJS` reference are timed by
//! the same sampler (`bench_measure::measure`) through the `BenchEngine`
//! interface, so the latency ratio reflects execution with no process-startup
//! component on either side and the runner owns every measurement. The CLI /
//! startup-subtraction machinery it replaces has been removed.
//!
//! Peak-memory comparison used to piggyback on a CLI process; a byte-level
//! in-process memory parity needs per-VM heap accounting that rs-quickjs does
//! not expose yet, so the `memory_ratio` column is reserved (`-`) for now.

use std::fs;

use tabled::Tabled;

use super::bench_engines::{BenchEngine, RsqjsEngine, make_reference};
use super::bench_measure::{self, MeasureConfig, MeasureStats, format_duration, ratio_values};
use super::cases::{self, BenchmarkCase};
use super::report_text;

pub const BUDGET_LABEL: &str = "1.00x";

const BUDGET_NUMERATOR: u128 = 100;
const BUDGET_DENOMINATOR: u128 = 100;
const STATUS_MEASURED: &str = "✅ measured";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_INVALID_BENCHMARK: &str = "❌ invalid benchmark";
const STATUS_TRACKED_EXCEPTION: &str = "🟡 tracked exception";
const STATUS_WITHIN_BUDGET: &str = "✅ within budget";
const BUDGET_WITHIN: &str = "✅ <= 1.00x";
const BUDGET_OVER: &str = "🟡 > 1.00x";
const BUDGET_INVALID: &str = "❌ invalid";
const BUDGET_NOT_AVAILABLE: &str = "🟡 unavailable";
const BUDGET_NOT_CONFIGURED: &str = "🟡 no reference";
const QUALITY_VALID: &str = "✅ valid";
const QUALITY_INVALID: &str = "❌ invalid";
const REFERENCE_NOT_CONFIGURED: &str = "🟡 not configured";
const REFERENCE_NOT_AVAILABLE: &str = "🟡 not available";
const NOT_MEASURED: &str = "-";
const DETAIL_COMPLETED: &str = "sequential benchmark completed";
const DETAIL_LATENCY_EXCEPTION: &str = "latency budget exception tracked";
const DETAIL_QUALITY_GATE: &str = "measurement quality gate failed";
const DETAIL_REFERENCE_COMPLETED: &str = "QuickJS reference completed";
const MAX_BENCHMARK_DETAIL_CHARS: usize = 80;

#[derive(Debug)]
pub struct BenchmarkReport {
    pub rows: Vec<BenchmarkRow>,
    pub measured: usize,
    pub in_process_measured: usize,
    pub failed: usize,
    pub invalid: usize,
    pub skipped: usize,
    pub over_latency_budget: usize,
    pub over_memory_budget: usize,
}

#[derive(Debug, Tabled)]
pub struct BenchmarkRow {
    benchmark: String,
    status: String,
    source: String,
    iterations: usize,
    rsqjs_eval: String,
    quickjs_eval: String,
    latency_ratio: String,
    latency_budget: String,
    memory_ratio: String,
    rsqjs_cv: String,
    quickjs_cv: String,
    quality: String,
    detail: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct BenchmarkCounts {
    measured: usize,
    in_process_measured: usize,
    failed: usize,
    invalid: usize,
    skipped: usize,
    over_latency_budget: usize,
}

#[derive(Debug)]
struct BenchmarkOutcome {
    row: BenchmarkRow,
    counts: BenchmarkCounts,
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
pub fn run() -> BenchmarkReport {
    let config = MeasureConfig::in_process_from_env();
    let reference = make_reference();
    let mut report = BenchmarkReport {
        rows: Vec::new(),
        measured: 0,
        in_process_measured: 0,
        failed: 0,
        invalid: 0,
        skipped: 0,
        over_latency_budget: 0,
        over_memory_budget: 0,
    };
    for case in cases::benchmark_cases() {
        let outcome = run_benchmark_case(&case, config, reference.as_deref());
        report.measured = report.measured.saturating_add(outcome.counts.measured);
        report.in_process_measured = report
            .in_process_measured
            .saturating_add(outcome.counts.in_process_measured);
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

fn run_benchmark_case(
    case: &BenchmarkCase,
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
) -> BenchmarkOutcome {
    let source = match fs::read_to_string(case.path) {
        Ok(source) => source,
        Err(error) => {
            return failed_outcome(case, &format!("failed to read '{}': {error}", case.path));
        }
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
    case: &BenchmarkCase,
    ours: MeasureStats,
    reference: ReferenceMeasurement,
) -> BenchmarkOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => measured_with_reference(case, ours, reference),
        ReferenceMeasurement::Failed(note) => reference_unavailable(case, ours, &note),
        ReferenceMeasurement::NotConfigured => measured_without_reference(case, ours),
    }
}

fn failed_with_reference(
    case: &BenchmarkCase,
    detail: &str,
    reference: ReferenceMeasurement,
) -> BenchmarkOutcome {
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
    case: &BenchmarkCase,
    quickjs_eval: String,
    quickjs_cv: String,
    quality: String,
    row_detail: &str,
) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: failed_row(
            case,
            NOT_MEASURED.to_owned(),
            quickjs_eval,
            NOT_MEASURED.to_owned(),
            quickjs_cv,
            quality,
            row_detail,
        ),
        counts: BenchmarkCounts {
            failed: 1,
            ..BenchmarkCounts::default()
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

fn measured_with_reference(
    case: &BenchmarkCase,
    ours: MeasureStats,
    reference: MeasureStats,
) -> BenchmarkOutcome {
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
    let over = budget.over_budget;
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: benchmark_status(over).to_owned(),
            source: case.path.to_owned(),
            iterations: report_iterations(ours),
            rsqjs_eval: format_duration(ours.median()),
            quickjs_eval: format_duration(reference.median()),
            latency_ratio: ratio_values(ours.median().as_nanos(), reference.median().as_nanos()),
            latency_budget: budget.label.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: ours.cv_percent_text(),
            quickjs_cv: reference.cv_percent_text(),
            quality: QUALITY_VALID.to_owned(),
            detail: benchmark_detail(&detail_text(over)),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            over_latency_budget: count_if(over),
            ..BenchmarkCounts::default()
        },
    }
}

fn measured_without_reference(case: &BenchmarkCase, ours: MeasureStats) -> BenchmarkOutcome {
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
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_MEASURED.to_owned(),
            source: case.path.to_owned(),
            iterations: report_iterations(ours),
            rsqjs_eval: format_duration(ours.median()),
            quickjs_eval: REFERENCE_NOT_CONFIGURED.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: BUDGET_NOT_CONFIGURED.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: ours.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: benchmark_detail(DETAIL_COMPLETED),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            skipped: 1,
            ..BenchmarkCounts::default()
        },
    }
}

/// The engine under test was measured, but the reference could not run this
/// script (e.g. an unsupported construct): report our number without a ratio
/// and note the reason, rather than failing the benchmark.
fn reference_unavailable(case: &BenchmarkCase, ours: MeasureStats, note: &str) -> BenchmarkOutcome {
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
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_MEASURED.to_owned(),
            source: case.path.to_owned(),
            iterations: report_iterations(ours),
            rsqjs_eval: format_duration(ours.median()),
            quickjs_eval: REFERENCE_NOT_AVAILABLE.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: BUDGET_NOT_AVAILABLE.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: ours.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: benchmark_detail(&format!("{DETAIL_COMPLETED}; reference error: {note}")),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            skipped: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn invalid_measurement_outcome(
    case: &BenchmarkCase,
    ours: MeasureStats,
    quickjs_eval: String,
    quickjs_cv: String,
    detail: &str,
    skipped_reference: bool,
) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_INVALID_BENCHMARK.to_owned(),
            source: case.path.to_owned(),
            iterations: report_iterations(ours),
            rsqjs_eval: format_duration(ours.median()),
            quickjs_eval,
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: BUDGET_INVALID.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: ours.cv_percent_text(),
            quickjs_cv,
            quality: QUALITY_INVALID.to_owned(),
            detail: benchmark_detail(detail),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            failed: 1,
            invalid: 1,
            skipped: count_if(skipped_reference),
            ..BenchmarkCounts::default()
        },
    }
}

fn failed_outcome(case: &BenchmarkCase, detail: &str) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: failed_row(
            case,
            NOT_MEASURED.to_owned(),
            NOT_MEASURED.to_owned(),
            NOT_MEASURED.to_owned(),
            NOT_MEASURED.to_owned(),
            NOT_MEASURED.to_owned(),
            detail,
        ),
        counts: BenchmarkCounts {
            failed: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn failed_row(
    case: &BenchmarkCase,
    rsqjs_eval: String,
    quickjs_eval: String,
    rsqjs_cv: String,
    quickjs_cv: String,
    quality: String,
    detail: &str,
) -> BenchmarkRow {
    BenchmarkRow {
        benchmark: case.id.to_owned(),
        status: STATUS_FAILED.to_owned(),
        source: case.path.to_owned(),
        iterations: 0,
        rsqjs_eval,
        quickjs_eval,
        latency_ratio: NOT_MEASURED.to_owned(),
        latency_budget: NOT_MEASURED.to_owned(),
        memory_ratio: NOT_MEASURED.to_owned(),
        rsqjs_cv,
        quickjs_cv,
        quality,
        detail: benchmark_detail(detail),
    }
}

fn benchmark_detail(detail: &str) -> String {
    report_text::table_detail_with_limit(detail, MAX_BENCHMARK_DETAIL_CHARS)
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

fn report_iterations(stats: MeasureStats) -> usize {
    usize::try_from(stats.total_iters()).unwrap_or(usize::MAX)
}

const fn budget_check(ours: u128, reference: u128) -> BudgetCheck {
    if reference == 0 {
        return BudgetCheck {
            label: BUDGET_NOT_AVAILABLE,
            over_budget: false,
        };
    }
    let over_budget =
        ours.saturating_mul(BUDGET_DENOMINATOR) > reference.saturating_mul(BUDGET_NUMERATOR);
    BudgetCheck {
        label: if over_budget {
            BUDGET_OVER
        } else {
            BUDGET_WITHIN
        },
        over_budget,
    }
}

const fn benchmark_status(over_latency_budget: bool) -> &'static str {
    if over_latency_budget {
        return STATUS_TRACKED_EXCEPTION;
    }
    STATUS_WITHIN_BUDGET
}

const fn count_if(condition: bool) -> usize {
    if condition { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::{BUDGET_OVER, BUDGET_WITHIN, RsqjsEngine, bench_measure, budget_check};
    use crate::bench_engines::BenchEngine;
    use std::time::Duration;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn marks_exact_budget_as_within_budget() -> TestResult {
        let check = budget_check(100, 100);
        ensure_bool(!check.over_budget, "exact budget must be accepted")?;
        ensure_text(check.label, BUDGET_WITHIN)
    }

    #[test]
    fn marks_above_budget_as_tracked_exception() -> TestResult {
        let check = budget_check(101, 100);
        ensure_bool(check.over_budget, "above budget must be tracked")?;
        ensure_text(check.label, BUDGET_OVER)
    }

    #[test]
    fn samples_engine_under_test_in_process() -> TestResult {
        let config =
            super::MeasureConfig::new(Duration::from_millis(5), Duration::from_millis(15), 3);
        let stats =
            bench_measure::measure(config, || RsqjsEngine.eval("let value = 40 + 2; value"))?;
        ensure_bool(
            stats.median() <= Duration::from_secs(1),
            "in-process eval should finish quickly",
        )?;
        ensure_bool(
            stats.total_iters() > 1,
            "sampler must auto-scale iterations",
        )
    }

    #[test]
    fn failed_benchmark_preserves_quickjs_measurement() -> TestResult {
        let case = crate::cases::BenchmarkCase {
            id: "failed-case",
            path: "tests/corpora/benchmarks/active/arithmetic_chain.js",
        };
        let reference = sample_stats()?;
        let outcome = super::failed_with_reference(
            &case,
            "rsqjs eval failed: sample",
            super::ReferenceMeasurement::Measured(reference),
        );
        ensure_text(&outcome.row.status, super::STATUS_FAILED)?;
        ensure_text(&outcome.row.rsqjs_eval, super::NOT_MEASURED)?;
        ensure_bool(
            outcome.row.quickjs_eval != super::NOT_MEASURED,
            "failed row must retain QuickJS timing",
        )?;
        ensure_bool(
            outcome.row.quickjs_cv != super::NOT_MEASURED,
            "failed row must retain QuickJS variation",
        )?;
        ensure_usize(outcome.counts.failed, 1)
    }

    #[test]
    fn failed_benchmark_sanitizes_multiline_details() -> TestResult {
        let case = crate::cases::BenchmarkCase {
            id: "failed-case",
            path: "tests/corpora/benchmarks/active/arithmetic_chain.js",
        };
        let outcome = super::failed_outcome(
            &case,
            "lexer error: unexpected character '\u{1b}'\n    at eval_script:12:4",
        );

        ensure_bool(
            !outcome.row.detail.contains('\n'),
            "detail must be one line",
        )?;
        ensure_bool(
            !outcome.row.detail.contains('\u{1b}'),
            "detail must escape controls",
        )?;
        ensure_bool(
            outcome.row.detail.contains("\\u{001B}"),
            "detail must preserve escaped control context",
        )
    }

    #[test]
    fn reference_unavailable_keeps_detail_compact() -> TestResult {
        let case = crate::cases::BenchmarkCase {
            id: "reference-unavailable",
            path: "tests/corpora/benchmarks/active/arithmetic_chain.js",
        };
        let note = "quickjs: Error: not a function\n    at <eval> (eval_script:19:35)";
        let outcome = super::reference_unavailable(&case, sample_stats()?, note);

        ensure_bool(
            outcome.row.detail.chars().count() <= super::MAX_BENCHMARK_DETAIL_CHARS + 3,
            "benchmark detail must stay compact enough for table rendering",
        )?;
        ensure_bool(
            !outcome.row.detail.contains('\n'),
            "benchmark detail must be one physical table line",
        )
    }

    fn sample_stats() -> Result<crate::bench_measure::MeasureStats, anyhow::Error> {
        let config =
            super::MeasureConfig::new(Duration::from_millis(0), Duration::from_millis(1), 3)
                .with_quality(Duration::ZERO, u32::MAX);
        bench_measure::measure(config, || {
            std::hint::black_box(42_u64);
            Ok::<(), anyhow::Error>(())
        })
    }

    fn ensure_text(actual: &str, expected: &str) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected '{expected}', got '{actual}'").into())
    }

    fn ensure_bool(actual: bool, message: &str) -> TestResult {
        if actual {
            return Ok(());
        }
        Err(message.to_owned().into())
    }

    fn ensure_usize(actual: usize, expected: usize) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected {expected}, got {actual}").into())
    }
}
