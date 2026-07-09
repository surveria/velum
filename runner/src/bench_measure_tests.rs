use super::{
    MeasureConfig, MeasureStats, MeasurementQuality, better_measurement, format_duration, isqrt,
    measure, median_of_sorted, ratio_values,
};
use std::{
    hint::black_box,
    thread,
    time::{Duration, Instant},
};

fn fixed_config() -> MeasureConfig {
    MeasureConfig::new(Duration::from_millis(20), Duration::from_millis(60), 8)
        .with_quality(Duration::from_nanos(1), 10_000)
}

fn quality(low_signal: bool, high_variance: bool) -> MeasurementQuality {
    MeasurementQuality {
        min_op_time: Duration::from_millis(1),
        max_cv_permille: 100,
        low_signal,
        high_variance,
        iteration_cap_reached: false,
    }
}

fn stats(cv_permille: u32, quality: MeasurementQuality) -> MeasureStats {
    MeasureStats {
        median: Duration::from_millis(2),
        cv_permille,
        iters_per_sample: 1,
        samples: 3,
        median_sample: Duration::from_millis(20),
        warmup_elapsed: Duration::from_millis(5),
        timed_run_elapsed: Duration::from_millis(60),
        quality,
    }
}

#[test]
fn isqrt_matches_reference() {
    for value in [0u128, 1, 2, 3, 4, 15, 16, 17, 1_000_000, 1_000_003] {
        let root = isqrt(value);
        assert!(root * root <= value, "root too large for {value}");
        assert!(
            (root + 1) * (root + 1) > value,
            "root too small for {value}"
        );
    }
}

#[test]
fn median_handles_even_and_odd() {
    assert_eq!(median_of_sorted(&[1, 2, 3]), 2);
    assert_eq!(median_of_sorted(&[10, 20, 30, 40]), 25);
    assert_eq!(median_of_sorted(&[]), 0);
}

#[test]
fn formats_duration_with_three_significant_figures() {
    assert_eq!(format_duration(Duration::from_micros(1_500)), "1.50 ms");
    assert_eq!(format_duration(Duration::from_nanos(365)), "365 ns");
}

#[test]
fn formats_ratio_below_and_above_one() {
    let below = ratio_values(
        Duration::from_micros(5).as_nanos(),
        Duration::from_micros(366).as_nanos(),
    );
    assert_eq!(below, "0.01x");
    let above = ratio_values(
        Duration::from_micros(250).as_nanos(),
        Duration::from_micros(100).as_nanos(),
    );
    assert_eq!(above, "2.50x");
}

#[test]
fn retry_selection_prefers_valid_measurement() -> anyhow::Result<()> {
    let invalid = stats(250, quality(false, true));
    let valid = stats(90, quality(false, false));
    ensure_bool(
        better_measurement(valid, invalid),
        "valid measurement should replace an invalid result",
    )?;
    ensure_bool(
        !better_measurement(invalid, valid),
        "invalid measurement should not replace a valid result",
    )
}

#[test]
fn retry_selection_prefers_less_noisy_invalid_measurement() -> anyhow::Result<()> {
    let noisy = stats(250, quality(false, true));
    let less_noisy = stats(120, quality(false, true));
    ensure_bool(
        better_measurement(less_noisy, noisy),
        "lower CV should replace an equally invalid result",
    )?;
    ensure_bool(
        !better_measurement(noisy, less_noisy),
        "higher CV should not replace a less noisy result",
    )
}

#[test]
fn measure_scales_iterations_and_is_repeatable() -> anyhow::Result<()> {
    let work = || -> anyhow::Result<()> {
        let mut acc = 0u64;
        for value in 0..2_000u64 {
            acc = acc.wrapping_add(value.wrapping_mul(value));
        }
        black_box(acc);
        Ok(())
    };
    let first = measure(fixed_config(), work)?;
    let second = measure(fixed_config(), work)?;
    ensure_bool(first.total_iters() > 1, "sampler did not scale iterations")?;
    let lo = first.median().as_nanos().min(second.median().as_nanos());
    let hi = first.median().as_nanos().max(second.median().as_nanos());
    ensure_bool(
        hi <= lo.saturating_mul(3).max(1),
        &format!("medians diverged: {lo} vs {hi}"),
    )
}

#[test]
fn measure_reports_low_variation_for_stable_work() -> anyhow::Result<()> {
    let start = Instant::now();
    let work = || -> anyhow::Result<()> {
        let mut acc = 0u64;
        for value in 0..1_500u64 {
            acc = acc.wrapping_add(value);
        }
        black_box(acc);
        Ok(())
    };
    let stats = measure(fixed_config(), work)?;
    black_box(start.elapsed());
    ensure_bool(stats.total_iters() > 0, "sampler produced no iterations")
}

#[test]
fn measure_marks_tiny_work_as_low_signal() -> anyhow::Result<()> {
    let config = MeasureConfig::new(Duration::from_millis(5), Duration::from_millis(15), 3)
        .with_quality(Duration::from_millis(1), 10_000);
    let stats = measure(config, || Ok(()))?;
    ensure_bool(stats.quality().low_signal(), "tiny work was not low signal")?;
    ensure_bool(!stats.quality().is_valid(), "tiny work was accepted")
}

#[test]
fn rejects_an_operation_above_the_duration_limit() -> anyhow::Result<()> {
    let config = MeasureConfig::new(Duration::ZERO, Duration::from_millis(1), 3)
        .with_quality(Duration::ZERO, u32::MAX)
        .with_budget(Duration::from_millis(1), Duration::from_millis(20));
    let result = measure(config, || {
        thread::sleep(Duration::from_millis(3));
        Ok(())
    });
    ensure_bool(result.is_err(), "overlong benchmark operation was repeated")
}

#[test]
fn total_budget_reduces_requested_sample_count() -> anyhow::Result<()> {
    let config = MeasureConfig::new(Duration::ZERO, Duration::from_secs(1), 20)
        .with_quality(Duration::ZERO, u32::MAX)
        .with_budget(Duration::from_millis(10), Duration::from_millis(12));
    let stats = measure(config, || {
        thread::sleep(Duration::from_millis(2));
        Ok(())
    })?;
    ensure_bool(
        stats.snapshot().samples < 20,
        "sampler ignored its total measurement budget",
    )?;
    ensure_bool(
        stats.snapshot().samples >= 3,
        "sampler did not retain the minimum robust sample count",
    )
}

fn ensure_bool(actual: bool, message: &str) -> anyhow::Result<()> {
    if actual {
        return Ok(());
    }
    Err(anyhow::anyhow!(message.to_owned()))
}
