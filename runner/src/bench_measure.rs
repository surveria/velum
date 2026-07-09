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
const ENV_ATTEMPTS: &str = "RSQJS_BENCH_ATTEMPTS";
const ENV_MAX_OP_MS: &str = "RSQJS_BENCH_MAX_OP_MS";
const ENV_MAX_TOTAL_MS: &str = "RSQJS_BENCH_MAX_TOTAL_MS";

const DEFAULT_MIN_TIME_MS: u64 = 500;
const DEFAULT_WARMUP_MS: u64 = 150;
const DEFAULT_SAMPLES: usize = 10;
const DEFAULT_MIN_OP_US: u64 = 1_000;
const DEFAULT_MAX_CV_PERCENT: u64 = 10;
const DEFAULT_ATTEMPTS: usize = 3;
const DEFAULT_MAX_OP_MS: u64 = 2_000;
const DEFAULT_MAX_TOTAL_MS: u64 = 3_000;
const HIGH_VARIANCE_MIN_TOTAL_MULTIPLIER: u32 = 2;

const MIN_SAMPLES: usize = 3;
const MIN_ATTEMPTS: usize = 1;
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
    attempts: usize,
    max_op_time: Duration,
    max_total: Duration,
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
            attempts: MIN_ATTEMPTS,
            max_op_time: Duration::from_millis(DEFAULT_MAX_OP_MS),
            max_total: Duration::from_millis(DEFAULT_MAX_TOTAL_MS),
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
        .with_attempts(env_usize(ENV_ATTEMPTS, DEFAULT_ATTEMPTS))
        .with_budget(
            Duration::from_millis(env_u64(ENV_MAX_OP_MS, DEFAULT_MAX_OP_MS)),
            Duration::from_millis(env_u64(ENV_MAX_TOTAL_MS, DEFAULT_MAX_TOTAL_MS)),
        )
    }

    pub const fn warmup(self) -> Duration {
        self.warmup
    }

    pub const fn min_total(self) -> Duration {
        self.min_total
    }

    pub const fn samples(self) -> usize {
        self.samples
    }

    pub const fn min_op_time(self) -> Duration {
        self.min_op_time
    }

    pub const fn max_cv_permille(self) -> u32 {
        self.max_cv_permille
    }

    pub const fn attempts(self) -> usize {
        self.attempts
    }

    #[must_use]
    pub const fn with_quality(mut self, min_op_time: Duration, max_cv_permille: u32) -> Self {
        self.min_op_time = min_op_time;
        self.max_cv_permille = max_cv_permille;
        self
    }

    #[must_use]
    pub const fn with_attempts(mut self, attempts: usize) -> Self {
        self.attempts = if attempts < MIN_ATTEMPTS {
            MIN_ATTEMPTS
        } else {
            attempts
        };
        self
    }

    #[must_use]
    pub const fn with_budget(mut self, max_op_time: Duration, max_total: Duration) -> Self {
        self.max_op_time = max_op_time;
        self.max_total = max_total;
        self
    }

    pub fn fingerprint(self) -> String {
        format!(
            "warmup_ns={},min_total_ns={},samples={},min_op_ns={},max_cv_permille={},attempts={},max_op_ns={},max_total_ns={}",
            self.warmup.as_nanos(),
            self.min_total.as_nanos(),
            self.samples,
            self.min_op_time.as_nanos(),
            self.max_cv_permille,
            self.attempts,
            self.max_op_time.as_nanos(),
            self.max_total.as_nanos(),
        )
    }

    const fn with_min_total_multiplier(mut self, multiplier: u32) -> Self {
        if let Some(min_total) = self.min_total.checked_mul(multiplier) {
            self.min_total = min_total;
        }
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
    warmup_elapsed: Duration,
    timed_run_elapsed: Duration,
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

    pub const fn cv_permille(&self) -> u32 {
        self.cv_permille
    }

    pub fn total_iters(&self) -> u64 {
        self.iters_per_sample
            .saturating_mul(self.sample_count_u64())
    }

    pub const fn median_sample(&self) -> Duration {
        self.median_sample
    }

    pub const fn warmup_elapsed(&self) -> Duration {
        self.warmup_elapsed
    }

    pub const fn timed_run_elapsed(&self) -> Duration {
        self.timed_run_elapsed
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
pub struct MeasureSnapshot {
    pub median: Duration,
    pub cv_permille: u32,
    pub iters_per_sample: u64,
    pub samples: usize,
    pub median_sample: Duration,
    pub warmup_elapsed: Duration,
    pub timed_run_elapsed: Duration,
    pub iteration_cap_reached: bool,
}

impl MeasureStats {
    pub const fn snapshot(self) -> MeasureSnapshot {
        MeasureSnapshot {
            median: self.median,
            cv_permille: self.cv_permille,
            iters_per_sample: self.iters_per_sample,
            samples: self.samples,
            median_sample: self.median_sample,
            warmup_elapsed: self.warmup_elapsed,
            timed_run_elapsed: self.timed_run_elapsed,
            iteration_cap_reached: self.quality.iteration_cap_reached(),
        }
    }

    pub const fn from_snapshot(snapshot: MeasureSnapshot, config: MeasureConfig) -> Self {
        Self {
            median: snapshot.median,
            cv_permille: snapshot.cv_permille,
            iters_per_sample: snapshot.iters_per_sample,
            samples: snapshot.samples,
            median_sample: snapshot.median_sample,
            warmup_elapsed: snapshot.warmup_elapsed,
            timed_run_elapsed: snapshot.timed_run_elapsed,
            quality: measurement_quality(
                config,
                snapshot.median.as_nanos(),
                snapshot.cv_permille,
                snapshot.iteration_cap_reached,
            ),
        }
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
    let deadline = Instant::now().checked_add(config.max_total);
    let mut best = measure_with_attempts(config, &mut op, deadline)?;
    if best.quality().is_valid() {
        return Ok(best);
    }
    if should_retry_with_larger_sample(best.quality()) && !deadline_expired(deadline) {
        let extended = config.with_min_total_multiplier(HIGH_VARIANCE_MIN_TOTAL_MULTIPLIER);
        let candidate = measure_with_attempts(extended, &mut op, deadline)?;
        if better_measurement(candidate, best) {
            best = candidate;
        }
    }
    Ok(best)
}

fn measure_with_attempts<F>(
    config: MeasureConfig,
    op: &mut F,
    deadline: Option<Instant>,
) -> anyhow::Result<MeasureStats>
where
    F: FnMut() -> anyhow::Result<()>,
{
    let mut best = measure_once(config, op, deadline)?;
    if best.quality().is_valid() {
        return Ok(best);
    }
    for _ in MIN_ATTEMPTS..config.attempts {
        if deadline_expired(deadline) {
            break;
        }
        let candidate = measure_once(config, op, deadline)?;
        if better_measurement(candidate, best) {
            best = candidate;
        }
        if best.quality().is_valid() {
            break;
        }
    }
    Ok(best)
}

const fn should_retry_with_larger_sample(quality: MeasurementQuality) -> bool {
    quality.high_variance() && !quality.low_signal() && !quality.iteration_cap_reached()
}

fn measure_once<F>(
    config: MeasureConfig,
    op: &mut F,
    deadline: Option<Instant>,
) -> anyhow::Result<MeasureStats>
where
    F: FnMut() -> anyhow::Result<()>,
{
    let warmup = warmup_and_estimate(config, op, deadline)?;
    let remaining = deadline.map(|deadline| deadline.saturating_duration_since(Instant::now()));
    let calibration = calibrate_iters(config, warmup.op_cost, remaining);
    let iters_u64 = clamp_u128_to_u64(calibration.iters_per_sample);

    let mut per_op = Vec::with_capacity(config.samples);
    let mut sample_times = Vec::with_capacity(config.samples);
    let sampling_start = Instant::now();
    for sample_index in 0..config.samples {
        if sample_index >= MIN_SAMPLES && deadline_expired(deadline) {
            break;
        }
        let start = Instant::now();
        for _ in 0..iters_u64 {
            op()?;
        }
        let elapsed = start.elapsed().as_nanos();
        let measured_op = elapsed / calibration.iters_per_sample.max(1);
        if measured_op > config.max_op_time.as_nanos() {
            anyhow::bail!(
                "benchmark operation exceeded maximum duration: {} ns > {} ns",
                measured_op,
                config.max_op_time.as_nanos()
            );
        }
        sample_times.push(elapsed);
        per_op.push(measured_op);
    }
    Ok(summarize(
        per_op,
        sample_times,
        iters_u64,
        config,
        calibration.capped,
        warmup.elapsed,
        sampling_start.elapsed(),
    ))
}

fn better_measurement(candidate: MeasureStats, current: MeasureStats) -> bool {
    let candidate_quality = candidate.quality();
    let current_quality = current.quality();
    if candidate_quality.is_valid() != current_quality.is_valid() {
        return candidate_quality.is_valid();
    }

    let candidate_score = quality_problem_count(candidate_quality);
    let current_score = quality_problem_count(current_quality);
    if candidate_score != current_score {
        return candidate_score < current_score;
    }

    if candidate.cv_permille() != current.cv_permille() {
        return candidate.cv_permille() < current.cv_permille();
    }

    candidate.median_sample() < current.median_sample()
}

const fn quality_problem_count(quality: MeasurementQuality) -> u8 {
    let mut count = 0u8;
    if quality.low_signal() {
        count += 1;
    }
    if quality.high_variance() {
        count += 1;
    }
    if quality.iteration_cap_reached() {
        count += 1;
    }
    count
}

#[derive(Debug, Clone, Copy)]
struct WarmupEstimate {
    op_cost: u128,
    elapsed: Duration,
}

fn warmup_and_estimate<F>(
    config: MeasureConfig,
    op: &mut F,
    deadline: Option<Instant>,
) -> anyhow::Result<WarmupEstimate>
where
    F: FnMut() -> anyhow::Result<()>,
{
    let start = Instant::now();
    let mut calls: u128 = 0;
    loop {
        let op_start = Instant::now();
        op()?;
        let op_elapsed = op_start.elapsed();
        if op_elapsed > config.max_op_time {
            anyhow::bail!(
                "benchmark operation exceeded maximum duration during warmup: {} ns > {} ns",
                op_elapsed.as_nanos(),
                config.max_op_time.as_nanos()
            );
        }
        calls = calls.saturating_add(1);
        if start.elapsed() >= config.warmup || deadline_expired(deadline) {
            break;
        }
    }
    let elapsed = start.elapsed();
    Ok(WarmupEstimate {
        op_cost: (elapsed.as_nanos().max(1) / calls.max(1)).max(1),
        elapsed,
    })
}

#[derive(Debug, Clone, Copy)]
struct Calibration {
    iters_per_sample: u128,
    capped: bool,
}

fn calibrate_iters(
    config: MeasureConfig,
    op_cost: u128,
    remaining_budget: Option<Duration>,
) -> Calibration {
    let samples = u128::try_from(config.samples.max(1)).unwrap_or(1);
    let requested_target = config.min_total.as_nanos() / samples;
    let target_per_sample = remaining_budget
        .map_or(requested_target, |remaining| {
            requested_target.min(remaining.as_nanos() / samples)
        })
        .max(op_cost);
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
    warmup_elapsed: Duration,
    timed_run_elapsed: Duration,
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
        samples: per_op.len(),
        median_sample: duration_from_nanos(median_sample),
        warmup_elapsed,
        timed_run_elapsed,
        quality: measurement_quality(config, median, cv_permille, iteration_cap_reached),
    }
}

fn deadline_expired(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
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
#[path = "bench_measure_tests.rs"]
mod tests;
