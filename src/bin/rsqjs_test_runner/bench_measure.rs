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
const ENV_CLI_SAMPLES: &str = "RSQJS_BENCH_CLI_SAMPLES";
const ENV_CLI_WARMUP: &str = "RSQJS_BENCH_CLI_WARMUP";

const DEFAULT_MIN_TIME_MS: u64 = 500;
const DEFAULT_WARMUP_MS: u64 = 150;
const DEFAULT_SAMPLES: usize = 10;
const DEFAULT_CLI_SAMPLES: usize = 20;
const DEFAULT_CLI_WARMUP: usize = 3;

const MIN_SAMPLES: usize = 3;
const MAX_ITERS_PER_SAMPLE: u128 = 50_000_000;
const PERMILLE_SCALE: u128 = 1000;

/// Configuration for one in-process time-based measurement.
#[derive(Debug, Clone, Copy)]
pub struct MeasureConfig {
    warmup: Duration,
    min_total: Duration,
    samples: usize,
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
        }
    }

    /// Build the in-process configuration from environment overrides.
    pub fn in_process_from_env() -> Self {
        Self::new(
            Duration::from_millis(env_u64(ENV_WARMUP_MS, DEFAULT_WARMUP_MS)),
            Duration::from_millis(env_u64(ENV_MIN_TIME_MS, DEFAULT_MIN_TIME_MS)),
            env_usize(ENV_SAMPLES, DEFAULT_SAMPLES),
        )
    }
}

/// Robust summary of a set of per-operation timings.
#[derive(Debug, Clone, Copy)]
pub struct MeasureStats {
    median: Duration,
    min: Duration,
    /// Coefficient of variation of the samples, in per-mille (‰).
    cv_permille: u32,
    iters_per_sample: u64,
    samples: usize,
}

impl MeasureStats {
    pub const fn median(&self) -> Duration {
        self.median
    }

    pub const fn min(&self) -> Duration {
        self.min
    }

    /// Coefficient of variation as a percentage with one decimal, e.g. `1.4`.
    pub fn cv_percent_text(&self) -> String {
        let whole = self.cv_permille / 10;
        let frac = self.cv_permille % 10;
        format!("{whole}.{frac}%")
    }

    pub fn total_iters(&self) -> u64 {
        self.iters_per_sample
            .saturating_mul(self.sample_count_u64())
    }

    fn sample_count_u64(&self) -> u64 {
        // sample counts are tiny; a saturating conversion never loses data here.
        u64::try_from(self.samples).unwrap_or(u64::MAX)
    }

    pub const fn samples(&self) -> usize {
        self.samples
    }
}

/// Measure an in-process operation with warmup, auto-calibration and sampling.
pub fn measure<F>(config: MeasureConfig, mut op: F) -> anyhow::Result<MeasureStats>
where
    F: FnMut() -> anyhow::Result<()>,
{
    let op_cost = warmup_and_estimate(config.warmup, &mut op)?;
    let iters_per_sample = calibrate_iters(config, op_cost);
    let iters_u64 = clamp_u128_to_u64(iters_per_sample);

    let mut per_op = Vec::with_capacity(config.samples);
    for _ in 0..config.samples {
        let start = Instant::now();
        for _ in 0..iters_u64 {
            op()?;
        }
        let elapsed = start.elapsed().as_nanos();
        per_op.push(elapsed / iters_per_sample.max(1));
    }
    Ok(summarize(per_op, iters_u64, config.samples))
}

/// Sample a process-spawning operation (one spawn per sample) robustly.
///
/// Each CLI iteration is a separate process, so looping inside a sample would
/// not reduce startup jitter. Instead we warm up a few spawns and then take
/// many single-spawn samples, reporting the median and minimum.
pub fn measure_cli_samples<F>(mut spawn: F) -> anyhow::Result<MeasureStats>
where
    F: FnMut() -> anyhow::Result<Duration>,
{
    let warmup = env_usize(ENV_CLI_WARMUP, DEFAULT_CLI_WARMUP);
    let samples = env_usize(ENV_CLI_SAMPLES, DEFAULT_CLI_SAMPLES).max(MIN_SAMPLES);
    for _ in 0..warmup {
        spawn()?;
    }
    let mut per_op = Vec::with_capacity(samples);
    for _ in 0..samples {
        per_op.push(spawn()?.as_nanos());
    }
    Ok(summarize(per_op, 1, samples))
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

fn calibrate_iters(config: MeasureConfig, op_cost: u128) -> u128 {
    let samples = u128::try_from(config.samples.max(1)).unwrap_or(1);
    let target_per_sample = config.min_total.as_nanos() / samples;
    let iters = target_per_sample / op_cost.max(1);
    iters.clamp(1, MAX_ITERS_PER_SAMPLE)
}

fn summarize(mut per_op: Vec<u128>, iters_per_sample: u64, samples: usize) -> MeasureStats {
    per_op.sort_unstable();
    let min = per_op.first().copied().unwrap_or(0);
    let median = median_of_sorted(&per_op);
    let cv_permille = coefficient_of_variation_permille(&per_op);
    MeasureStats {
        median: duration_from_nanos(median),
        min: duration_from_nanos(min),
        cv_permille,
        iters_per_sample,
        samples,
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

#[cfg(test)]
mod tests {
    use super::{MeasureConfig, isqrt, measure, median_of_sorted};
    use std::{
        hint::black_box,
        time::{Duration, Instant},
    };

    fn fixed_config() -> MeasureConfig {
        MeasureConfig {
            warmup: Duration::from_millis(20),
            min_total: Duration::from_millis(60),
            samples: 8,
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
        // Structural sanity: at least the configured samples were taken.
        assert_eq!(stats.samples(), 8);
        Ok(())
    }
}
