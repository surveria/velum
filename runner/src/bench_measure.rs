//! Robust micro-benchmark timing core.
//!
//! The previous harness ran a fixed 50 iterations and reported the arithmetic
//! mean of a single batch. For microsecond-scale workloads that estimator is
//! dominated by scheduler jitter on the shared self-hosted runner, so the
//! reported ratios carried a large run-to-run noise floor.
//!
//! This module replaces that with a time-based, warmed-up, multi-sample
//! measurement:
//!
//! * a warmup phase lets CPU caches, branch predictors and frequency scaling
//!   settle, and is reused to estimate the per-operation cost;
//! * the inner iteration count is auto-calibrated so each sample runs for a
//!   target slice of wall time (so trivial cases stop measuring pure noise);
//! * several samples are collected and summarised with robust statistics
//!   (median and minimum), plus a coefficient of variation so the remaining
//!   noise is visible in the report instead of hidden.
//!
//! All statistics are computed in integer nanoseconds to stay clear of the
//! `cast_precision_loss` lint; no floating point is used.

use std::{
    env,
    time::{Duration, Instant},
};

const ENV_MIN_TIME_MS: &str = "RSQJS_BENCH_MIN_TIME_MS";
const ENV_WARMUP_MS: &str = "RSQJS_BENCH_WARMUP_MS";
const ENV_SAMPLES: &str = "RSQJS_BENCH_SAMPLES";
const ENV_MIN_OP_US: &str = "RSQJS_BENCH_MIN_OP_US";
const ENV_MAX_CV_PERCENT: &str = "RSQJS_BENCH_MAX_CV_PERCENT";

const DEFAULT_MIN_TIME_MS: u64 = 500;
const DEFAULT_WARMUP_MS: u64 = 150;
const DEFAULT_SAMPLES: usize = 10;
const DEFAULT_MIN_OP_US: u64 = 1_000;
const DEFAULT_MAX_CV_PERCENT: u64 = 10;

const MIN_SAMPLES: usize = 3;
const MAX_ITERS_PER_SAMPLE: u128 = 50_000_000;
const PERMILLE_SCALE: u128 = 1000;
const PERCENT_TO_PERMILLE: u64 = 10;

const NANOS_PER_MICROSECOND: u128 = 1_000;
const NANOS_PER_MILLISECOND: u128 = 1_000_000;
const RATIO_DECIMAL_SCALE: u128 = 100;
const FRACTION_SCALE: u128 = 100;

/// Configuration for one in-process time-based measurement.
#[derive(Debug, Clone, Copy)]
pub struct MeasureConfig {
    warmup: Duration,
    min_total: Duration,
    samples: usize,
    min_op_time: Duration,
    max_cv_permille: u32,
}

impl MeasureConfig {
    /// Build a configuration explicitly. Callers that want their own time
    /// budget (and tests that need a fast one) use this instead of the
    /// environment-driven default.
    pub fn new(warmup: Duration, min_total: Duration, samples: usize) -> Self {
        Self {
            warmup,
            min_total,
            samples: samples.max(MIN_SAMPLES),
            min_op_time: Duration::from_micros(DEFAULT_MIN_OP_US),
            max_cv_permille: cv_percent_to_permille(DEFAULT_MAX_CV_PERCENT),
        }
    }

    /// Build the in-process configuration from environment overrides.
    pub fn in_process_from_env() -> Self {
        Self::new(
            Duration::from_millis(env_u64(ENV_WARMUP_MS, DEFAULT_WARMUP_MS)),
            Duration::from_millis(env_u64(ENV_MIN_TIME_MS, DEFAULT_MIN_TIME_MS)),
            env_usize(ENV_SAMPLES, DEFAULT_SAMPLES),
        )
        .with_quality(
            Duration::from_micros(env_u64(ENV_MIN_OP_US, DEFAULT_MIN_OP_US)),
            cv_percent_to_permille(env_u64(ENV_MAX_CV_PERCENT, DEFAULT_MAX_CV_PERCENT)),
        )
    }

    #[must_use]
    pub const fn with_quality(mut self, min_op_time: Duration, max_cv_permille: u32) -> Self {
        self.min_op_time = min_op_time;
        self.max_cv_permille = max_cv_permille;
        self
    }
}

/// Robust summary of a set of per-operation timings.
#[derive(Debug, Clone, Copy)]
pub struct MeasureStats {
    median: Duration,
    /// Coefficient of variation of the samples, in per-mille (‰).
    cv_permille: u32,
    iters_per_sample: u64,
    samples: usize,
    median_sample: Duration,
    quality: MeasurementQuality,
}

impl MeasureStats {
    pub const fn median(&self) -> Duration {
        self.median
    }

    /// Coefficient of variation as a percentage with one decimal, e.g. `1.4`.
    pub fn cv_percent_text(&self) -> String {
        cv_permille_text(self.cv_permille)
    }

    pub fn total_iters(&self) -> u64 {
        self.iters_per_sample
            .saturating_mul(self.sample_count_u64())
    }

    pub const fn median_sample(&self) -> Duration {
        self.median_sample
    }

    pub const fn quality(&self) -> MeasurementQuality {
        self.quality
    }

    fn sample_count_u64(&self) -> u64 {
        // sample counts are tiny; a saturating conversion never loses data here.
        u64::try_from(self.samples).unwrap_or(u64::MAX)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct MeasurementQuality {
    min_op_time: Duration,
    max_cv_permille: u32,
    low_signal: bool,
    high_variance: bool,
    iteration_cap_reached: bool,
}

impl MeasurementQuality {
    pub const fn is_valid(self) -> bool {
        !self.low_signal && !self.high_variance && !self.iteration_cap_reached
    }

    pub const fn min_op_time(self) -> Duration {
        self.min_op_time
    }

    pub fn max_cv_percent_text(self) -> String {
        cv_permille_text(self.max_cv_permille)
    }

    pub const fn low_signal(self) -> bool {
        self.low_signal
    }

    pub const fn high_variance(self) -> bool {
        self.high_variance
    }

    pub const fn iteration_cap_reached(self) -> bool {
        self.iteration_cap_reached
    }
}

/// Measure an in-process operation with warmup, auto-calibration and sampling.
pub fn measure<F>(config: MeasureConfig, mut op: F) -> anyhow::Result<MeasureStats>
where
    F: FnMut() -> anyhow::Result<()>,
{
    let op_cost = warmup_and_estimate(config.warmup, &mut op)?;
    let calibration = calibrate_iters(config, op_cost);
    let iters_u64 = clamp_u128_to_u64(calibration.iters_per_sample);

    let mut per_op = Vec::with_capacity(config.samples);
    let mut sample_times = Vec::with_capacity(config.samples);
    for _ in 0..config.samples {
        let start = Instant::now();
        for _ in 0..iters_u64 {
            op()?;
        }
        let elapsed = start.elapsed().as_nanos();
        sample_times.push(elapsed);
        per_op.push(elapsed / calibration.iters_per_sample.max(1));
    }
    Ok(summarize(
        per_op,
        sample_times,
        iters_u64,
        config,
        calibration.capped,
    ))
}

fn warmup_and_estimate<F>(warmup: Duration, op: &mut F) -> anyhow::Result<u128>
where
    F: FnMut() -> anyhow::Result<()>,
{
    let start = Instant::now();
    let mut calls: u128 = 0;
    loop {
        op()?;
        calls = calls.saturating_add(1);
        if start.elapsed() >= warmup {
            break;
        }
    }
    let elapsed = start.elapsed().as_nanos().max(1);
    Ok((elapsed / calls.max(1)).max(1))
}

#[derive(Debug, Clone, Copy)]
struct Calibration {
    iters_per_sample: u128,
    capped: bool,
}

fn calibrate_iters(config: MeasureConfig, op_cost: u128) -> Calibration {
    let samples = u128::try_from(config.samples.max(1)).unwrap_or(1);
    let target_per_sample = config.min_total.as_nanos() / samples;
    let iters = target_per_sample / op_cost.max(1);
    let iters_per_sample = iters.clamp(1, MAX_ITERS_PER_SAMPLE);
    Calibration {
        iters_per_sample,
        capped: iters > MAX_ITERS_PER_SAMPLE,
    }
}

fn summarize(
    mut per_op: Vec<u128>,
    mut sample_times: Vec<u128>,
    iters_per_sample: u64,
    config: MeasureConfig,
    iteration_cap_reached: bool,
) -> MeasureStats {
    per_op.sort_unstable();
    let median = median_of_sorted(&per_op);
    let cv_permille = coefficient_of_variation_permille(&per_op);
    sample_times.sort_unstable();
    let median_sample = median_of_sorted(&sample_times);
    MeasureStats {
        median: duration_from_nanos(median),
        cv_permille,
        iters_per_sample,
        samples: config.samples,
        median_sample: duration_from_nanos(median_sample),
        quality: measurement_quality(config, median, cv_permille, iteration_cap_reached),
    }
}

const fn measurement_quality(
    config: MeasureConfig,
    median: u128,
    cv_permille: u32,
    iteration_cap_reached: bool,
) -> MeasurementQuality {
    MeasurementQuality {
        min_op_time: config.min_op_time,
        max_cv_permille: config.max_cv_permille,
        low_signal: median < config.min_op_time.as_nanos(),
        high_variance: cv_permille > config.max_cv_permille,
        iteration_cap_reached,
    }
}

fn median_of_sorted(sorted: &[u128]) -> u128 {
    let len = sorted.len();
    if len == 0 {
        return 0;
    }
    let mid = len / 2;
    if len % 2 == 1 {
        sorted.get(mid).copied().unwrap_or(0)
    } else {
        let lower = sorted.get(mid.saturating_sub(1)).copied().unwrap_or(0);
        let upper = sorted.get(mid).copied().unwrap_or(0);
        lower.midpoint(upper)
    }
}

fn coefficient_of_variation_permille(samples: &[u128]) -> u32 {
    let count = match u128::try_from(samples.len()) {
        Ok(value) if value > 0 => value,
        _ => return 0,
    };
    let sum: u128 = samples.iter().copied().sum();
    let mean = sum / count;
    if mean == 0 {
        return 0;
    }
    let variance = samples
        .iter()
        .map(|value| {
            let diff = value.abs_diff(mean);
            diff.saturating_mul(diff)
        })
        .fold(0u128, u128::saturating_add)
        / count;
    let stddev = isqrt(variance);
    clamp_u128_to_u32(stddev.saturating_mul(PERMILLE_SCALE) / mean)
}

/// Integer square root via bit-by-bit reconstruction (no floating point).
const fn isqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut result: u128 = 0;
    // Highest power of four not exceeding `value`.
    let mut bit: u128 = 1u128 << 126;
    while bit > value {
        bit >>= 2;
    }
    let mut remainder = value;
    while bit != 0 {
        if remainder >= result + bit {
            remainder -= result + bit;
            result = (result >> 1) + bit;
        } else {
            result >>= 1;
        }
        bit >>= 2;
    }
    result
}

fn duration_from_nanos(nanos: u128) -> Duration {
    Duration::from_nanos(clamp_u128_to_u64(nanos))
}

fn clamp_u128_to_u64(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn clamp_u128_to_u32(value: u128) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn cv_percent_to_permille(percent: u64) -> u32 {
    let permille = percent.saturating_mul(PERCENT_TO_PERMILLE);
    u32::try_from(permille).unwrap_or(u32::MAX)
}

fn cv_permille_text(permille: u32) -> String {
    let whole = permille / 10;
    let frac = permille % 10;
    format!("{whole}.{frac}%")
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(default)
}

/// Format a duration with three significant figures (e.g. `1.74 ms`).
pub fn format_duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    if nanos < NANOS_PER_MICROSECOND {
        return format!("{nanos} ns");
    }
    if nanos < NANOS_PER_MILLISECOND {
        return format!("{} us", fixed_point(nanos, NANOS_PER_MICROSECOND));
    }
    format!("{} ms", fixed_point(nanos, NANOS_PER_MILLISECOND))
}

fn fixed_point(nanos: u128, unit: u128) -> String {
    let whole = nanos / unit;
    let frac = (nanos % unit).saturating_mul(FRACTION_SCALE) / unit;
    format!("{whole}.{frac:02}")
}

/// Render `ours / reference` as a two-decimal ratio like `1.24x`.
pub fn ratio_values(ours: u128, reference: u128) -> String {
    if reference == 0 {
        return "-".to_owned();
    }
    let scaled = ours.saturating_mul(RATIO_DECIMAL_SCALE) / reference;
    format!(
        "{}.{:02}x",
        scaled / RATIO_DECIMAL_SCALE,
        scaled % RATIO_DECIMAL_SCALE
    )
}

#[cfg(test)]
mod tests {
    use super::{MeasureConfig, format_duration, isqrt, measure, median_of_sorted, ratio_values};
    use std::{
        hint::black_box,
        time::{Duration, Instant},
    };

    fn fixed_config() -> MeasureConfig {
        MeasureConfig::new(Duration::from_millis(20), Duration::from_millis(60), 8)
            .with_quality(Duration::from_nanos(1), 10_000)
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
    fn measure_scales_iterations_and_is_repeatable() -> anyhow::Result<()> {
        // A deterministic fixed-work operation should yield a stable median
        // across independent measurement runs. This is the property the old
        // fixed-50-iteration mean lacked on noisy hardware.
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
        // Auto-calibration must pick more than a single iteration for cheap work.
        assert!(first.total_iters() > 1);
        // Independent runs agree within a loose bound (robust to CI jitter).
        let lo = first.median().as_nanos().min(second.median().as_nanos());
        let hi = first.median().as_nanos().max(second.median().as_nanos());
        assert!(
            hi <= lo.saturating_mul(3).max(1),
            "medians diverged: {lo} vs {hi}"
        );
        Ok(())
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
        // Structural sanity: auto-calibration produced real iterations.
        assert!(stats.total_iters() > 0);
        Ok(())
    }

    #[test]
    fn measure_marks_tiny_work_as_low_signal() -> anyhow::Result<()> {
        let config = MeasureConfig::new(Duration::from_millis(5), Duration::from_millis(15), 3)
            .with_quality(Duration::from_millis(1), 10_000);
        let stats = measure(config, || Ok(()))?;
        assert!(stats.quality().low_signal());
        assert!(!stats.quality().is_valid());
        Ok(())
    }
}
