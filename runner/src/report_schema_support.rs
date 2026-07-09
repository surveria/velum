use std::{env, fs, time::Duration};

use anyhow::{Context as _, bail};

use crate::{
    bench_measure::MeasureConfig,
    report_schema::{
        BenchmarkConfiguration, BenchmarkSet, EnvironmentInfo, FeatureSelection, InputAvailability,
        NO_VALUE, QuickjsBaselineMode, ReportMode, RunConfiguration, Test262Mode,
    },
};

const BENCHMARK_FILTER_ENV: &str = "RSQJS_BENCH_FILTER";
const BENCHMARK_SET_ENV: &str = "RSQJS_BENCH_SET";
const QUICKJS_BASELINE_ENV: &str = "RSQJS_QUICKJS_BASELINE";
const TEST262_RUN_ALL_ENV: &str = "RSQJS_TEST262_RUN_ALL";
const TEST262_PATH_FILTER_ENV: &str = "RSQJS_TEST262_PATH_FILTER";
const TEST262_FLAG_FILTER_ENV: &str = "RSQJS_TEST262_FLAG_FILTER";

impl EnvironmentInfo {
    pub fn capture() -> Self {
        let available_parallelism = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .map_or(1, usize_to_u64);
        Self {
            operating_system: env::consts::OS.to_owned(),
            architecture: env::consts::ARCH.to_owned(),
            available_parallelism,
            build_profile: if cfg!(debug_assertions) {
                "debug".to_owned()
            } else {
                "release".to_owned()
            },
            kernel_release: read_trimmed("/proc/sys/kernel/osrelease"),
            cpu_model: cpu_model(),
            cpu_affinity: proc_status_value("Cpus_allowed_list"),
            scaling_governor: read_trimmed("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
        }
    }
}

impl RunConfiguration {
    pub fn capture(
        quickjs_configured: bool,
        test262_configured: bool,
        report_mode: ReportMode,
        jetstream_enabled: bool,
    ) -> Self {
        let benchmark = MeasureConfig::in_process_from_env();
        Self {
            report_mode,
            jetstream: feature_selection(jetstream_enabled),
            quickjs_differential: input_availability(quickjs_configured),
            test262: input_availability(test262_configured),
            test262_mode: if env::var(TEST262_RUN_ALL_ENV)
                .is_ok_and(|value| is_truthy(value.trim()))
            {
                Test262Mode::Full
            } else {
                Test262Mode::Manifest
            },
            test262_path_filters: env_list(TEST262_PATH_FILTER_ENV),
            test262_flag_filters: env_list(TEST262_FLAG_FILTER_ENV),
            benchmark_set: benchmark_set(),
            benchmark_filter: non_empty_env(BENCHMARK_FILTER_ENV),
            quickjs_baseline: quickjs_baseline_mode(),
            benchmark: BenchmarkConfiguration {
                reference_quickjs_compiled: cfg!(feature = "reference-quickjs"),
                warmup_duration_ns: duration_ns(benchmark.warmup()),
                minimum_sample_duration_ns: duration_ns(benchmark.min_total()),
                samples: usize_to_u64(benchmark.samples()),
                minimum_operation_duration_ns: duration_ns(benchmark.min_op_time()),
                maximum_cv_permille: benchmark.max_cv_permille(),
                attempts: usize_to_u64(benchmark.attempts()),
            },
        }
    }
}

fn benchmark_set() -> BenchmarkSet {
    match non_empty_env(BENCHMARK_SET_ENV).as_deref() {
        None | Some("full") => BenchmarkSet::Full,
        Some("sentinel") => BenchmarkSet::Sentinel,
        Some(_) => BenchmarkSet::Invalid,
    }
}

fn quickjs_baseline_mode() -> QuickjsBaselineMode {
    match non_empty_env(QUICKJS_BASELINE_ENV).as_deref() {
        None | Some("read") => QuickjsBaselineMode::Read,
        Some("off") => QuickjsBaselineMode::Off,
        Some("require") => QuickjsBaselineMode::Require,
        Some("refresh") => QuickjsBaselineMode::Refresh,
        Some(_) => QuickjsBaselineMode::Invalid,
    }
}

const fn feature_selection(enabled: bool) -> FeatureSelection {
    if enabled {
        return FeatureSelection::Enabled;
    }
    FeatureSelection::Disabled
}

const fn input_availability(configured: bool) -> InputAvailability {
    if configured {
        return InputAvailability::Configured;
    }
    InputAvailability::NotConfigured
}

pub fn labeled_count(value: &str) -> anyhow::Result<u64> {
    let Some(count) = value.split_whitespace().next() else {
        bail!("missing count in '{value}'");
    };
    count
        .parse::<u64>()
        .with_context(|| format!("invalid count in '{value}'"))
}

pub fn optional_duration(value: &str) -> anyhow::Result<Option<u64>> {
    if value == NO_VALUE {
        return Ok(None);
    }
    parse_duration(value)
        .map(Some)
        .with_context(|| format!("invalid duration '{value}'"))
}

pub fn parse_duration(value: &str) -> Option<u64> {
    let mut fields = value.split_whitespace();
    let number = fields.next()?;
    let unit = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    let scale = match unit {
        "ns" => 1,
        "us" => 1_000,
        "ms" => 1_000_000,
        "s" => 1_000_000_000,
        _ => return None,
    };
    parse_scaled_decimal(number, scale)
}

pub fn optional_ratio(value: &str) -> anyhow::Result<Option<u64>> {
    if value == NO_VALUE {
        return Ok(None);
    }
    let number = value
        .strip_suffix('x')
        .with_context(|| format!("invalid ratio '{value}'"))?;
    parse_scaled_decimal(number, 100)
        .map(Some)
        .with_context(|| format!("invalid ratio '{value}'"))
}

pub fn optional_cv_permille(value: &str) -> anyhow::Result<Option<u32>> {
    if value == NO_VALUE {
        return Ok(None);
    }
    let number = value
        .strip_suffix('%')
        .with_context(|| format!("invalid coefficient of variation '{value}'"))?;
    let parsed = parse_scaled_decimal(number, 10)
        .with_context(|| format!("invalid coefficient of variation '{value}'"))?;
    Ok(Some(u32::try_from(parsed).unwrap_or(u32::MAX)))
}

pub fn duration_ns(duration: Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

pub fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn parse_scaled_decimal(value: &str, scale: u64) -> Option<u64> {
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    let whole = whole.parse::<u64>().ok()?.checked_mul(scale)?;
    if fraction.is_empty() {
        return Some(whole);
    }
    let fraction_value = fraction.parse::<u64>().ok()?;
    let divisor = 10u64.checked_pow(u32::try_from(fraction.len()).ok()?)?;
    let scaled_fraction = fraction_value.checked_mul(scale)?.checked_div(divisor)?;
    whole.checked_add(scaled_fraction)
}

fn env_list(name: &str) -> Vec<String> {
    non_empty_env(name)
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn non_empty_env(name: &str) -> Option<String> {
    let value = env::var(name).ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_owned())
}

fn is_truthy(value: &str) -> bool {
    matches!(value, "1" | "true" | "yes" | "on")
}

fn read_trimmed(path: &str) -> Option<String> {
    let value = fs::read_to_string(path).ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(value.to_owned())
}

fn cpu_model() -> Option<String> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;
    cpuinfo.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim() != "model name" {
            return None;
        }
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        Some(value.to_owned())
    })
}

fn proc_status_value(key: &str) -> Option<String> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name != key {
            return None;
        }
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        Some(value.to_owned())
    })
}
