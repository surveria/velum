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
    let reference = super::ReferenceSample {
        stats: sample_stats()?,
        elapsed: Some(Duration::from_millis(1)),
        source: super::REFERENCE_SOURCE_LIVE,
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
    ensure_text(&outcome.row.reference_source, "live refresh")?;
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
    let config = super::MeasureConfig::new(Duration::from_millis(0), Duration::from_millis(1), 3)
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
