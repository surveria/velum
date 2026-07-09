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
    let config = super::MeasureConfig::new(Duration::from_millis(5), Duration::from_millis(15), 3);
    let stats = bench_measure::measure(config, || RsqjsEngine.eval("let value = 40 + 2; value"))?;
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
    let case = crate::cases::BenchmarkCase::cold(
        "failed-case",
        "tests/corpora/benchmarks/active/arithmetic_chain.js",
    );
    let reference = super::timing::Timed {
        value: sample_stats()?,
        elapsed: Duration::from_millis(1),
    };
    let outcome = super::failed_with_reference(
        &case,
        "rsqjs eval failed: sample",
        "1.00 ms".to_owned(),
        super::ReferenceMeasurement::Measured(reference),
        "2.00 ms".to_owned(),
    );
    ensure_text(&outcome.row.status, super::STATUS_FAILED)?;
    ensure_text(&outcome.row.case_elapsed, "2.00 ms")?;
    ensure_text(&outcome.row.rsqjs_measure, "1.00 ms")?;
    ensure_text(&outcome.row.quickjs_measure, "1.00 ms")?;
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

fn sample_stats() -> Result<crate::bench_measure::MeasureStats, anyhow::Error> {
    let config = super::MeasureConfig::new(Duration::ZERO, Duration::from_millis(1), 3)
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
