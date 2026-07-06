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

pub const BUDGET_LABEL: &str = "1.00x";

const BUDGET_NUMERATOR: u128 = 100;
const BUDGET_DENOMINATOR: u128 = 100;
const STATUS_MEASURED: &str = "✅ measured";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_TRACKED_EXCEPTION: &str = "🟡 tracked exception";
const STATUS_WITHIN_BUDGET: &str = "✅ within budget";
const BUDGET_WITHIN: &str = "✅ <= 1.00x";
const BUDGET_OVER: &str = "🟡 > 1.00x";
const BUDGET_NOT_AVAILABLE: &str = "🟡 unavailable";
const BUDGET_NOT_CONFIGURED: &str = "🟡 no reference";
const REFERENCE_NOT_CONFIGURED: &str = "🟡 not configured";
const REFERENCE_NOT_AVAILABLE: &str = "🟡 not available";
const NOT_MEASURED: &str = "-";
const DETAIL_COMPLETED: &str = "sequential benchmark completed";
const DETAIL_LATENCY_EXCEPTION: &str = "latency budget exception tracked";

#[derive(Debug)]
pub struct BenchmarkReport {
    pub rows: Vec<BenchmarkRow>,
    pub measured: usize,
    pub in_process_measured: usize,
    pub failed: usize,
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
    detail: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct BenchmarkCounts {
    measured: usize,
    in_process_measured: usize,
    failed: usize,
    skipped: usize,
    over_latency_budget: usize,
}

#[derive(Debug)]
struct BenchmarkOutcome {
    row: BenchmarkRow,
    counts: BenchmarkCounts,
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
    let ours = match bench_measure::measure(config, || RsqjsEngine.eval(&source)) {
        Ok(stats) => stats,
        Err(error) => return failed_outcome(case, &error.to_string()),
    };
    let Some(reference) = reference else {
        return measured_without_reference(case, ours);
    };
    match bench_measure::measure(config, || reference.eval(&source)) {
        Ok(reference_stats) => measured_with_reference(case, ours, reference_stats),
        Err(error) => reference_unavailable(case, ours, &format!("{}: {error}", reference.label())),
    }
}

fn measured_with_reference(
    case: &BenchmarkCase,
    ours: MeasureStats,
    reference: MeasureStats,
) -> BenchmarkOutcome {
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
            detail: detail_text(over),
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
            detail: DETAIL_COMPLETED.to_owned(),
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
            detail: format!("{DETAIL_COMPLETED}; reference error: {note}"),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            skipped: 1,
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
    rsqjs_cv: String,
    detail: &str,
) -> BenchmarkRow {
    BenchmarkRow {
        benchmark: case.id.to_owned(),
        status: STATUS_FAILED.to_owned(),
        source: case.path.to_owned(),
        iterations: 0,
        rsqjs_eval,
        quickjs_eval: NOT_MEASURED.to_owned(),
        latency_ratio: NOT_MEASURED.to_owned(),
        latency_budget: NOT_MEASURED.to_owned(),
        memory_ratio: NOT_MEASURED.to_owned(),
        rsqjs_cv,
        quickjs_cv: NOT_MEASURED.to_owned(),
        detail: detail.to_owned(),
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
}
