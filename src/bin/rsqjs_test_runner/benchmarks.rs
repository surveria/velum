use std::{
    fs,
    hint::black_box,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{self, Command},
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail};
use rs_quickjs::Runtime;
use tabled::Tabled;

use super::cases::{self, BenchmarkCase};

pub const BUDGET_LABEL: &str = "1.00x";

const BENCH_ITERATIONS: usize = 50;
const BUDGET_NUMERATOR: u128 = 100;
const BUDGET_DENOMINATOR: u128 = 100;
const GNU_TIME_PATH: &str = "/usr/bin/time";
const MEMORY_UNIT: &str = "KiB";
const NANOS_PER_MICROSECOND: u128 = 1_000;
const NANOS_PER_MILLISECOND: u128 = 1_000_000;
const RATIO_DECIMAL_SCALE: u128 = 100;
const REASON_ENGINE_ENV_MISSING: &str = "set RSQJS_ENGINE=/path/to/rsqjs to enable benchmarks";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_MEASURED: &str = "✅ measured";
const STATUS_NOT_AVAILABLE: &str = "🟡 not available";
const STATUS_NOT_CONFIGURED: &str = "🟡 not configured";
const STATUS_TRACKED_EXCEPTION: &str = "🟡 tracked exception";
const STATUS_WITHIN_BUDGET: &str = "✅ within budget";
const BUDGET_NOT_CONFIGURED: &str = "🟡 no reference";
const BUDGET_NOT_AVAILABLE: &str = "🟡 unavailable";
const BUDGET_OVER: &str = "🟡 > 1.00x";
const BUDGET_WITHIN: &str = "✅ <= 1.00x";
const DETAIL_COMPLETED: &str = "sequential benchmark completed";
const DETAIL_LATENCY_EXCEPTION: &str = "latency budget exception tracked";
const DETAIL_MEMORY_EXCEPTION: &str = "memory budget exception tracked";

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
    rsqjs_in_process_avg: String,
    rsqjs_compile_avg: String,
    rsqjs_compiled_eval_avg: String,
    rsqjs_cli_avg: String,
    quickjs_cli_avg: String,
    latency_ratio: String,
    latency_budget: String,
    rsqjs_peak_rss: String,
    quickjs_peak_rss: String,
    memory_ratio: String,
    memory_budget: String,
    detail: String,
}

#[derive(Debug)]
struct BenchmarkOutcome {
    row: BenchmarkRow,
    counts: BenchmarkCounts,
}

#[derive(Debug, Clone, Copy, Default)]
struct BenchmarkCounts {
    measured: usize,
    in_process_measured: usize,
    failed: usize,
    skipped: usize,
    over_latency_budget: usize,
    over_memory_budget: usize,
}

#[derive(Debug, Clone, Copy)]
struct InProcessMeasurements {
    cold_eval: Duration,
    compile: Duration,
    compiled_eval: Duration,
}

#[derive(Debug, Clone)]
enum MemoryMeasurement {
    Measured(u64),
    NotConfigured,
    Unavailable(String),
}

#[derive(Debug, Clone, Copy)]
struct BudgetCheck {
    label: &'static str,
    over_budget: bool,
}

#[must_use]
pub fn run(quickjs: Option<&Path>, engine: Option<&Path>) -> BenchmarkReport {
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
        let outcome = run_benchmark_case(&case, quickjs, engine);
        report.measured = report.measured.saturating_add(outcome.counts.measured);
        report.in_process_measured = report
            .in_process_measured
            .saturating_add(outcome.counts.in_process_measured);
        report.failed = report.failed.saturating_add(outcome.counts.failed);
        report.skipped = report.skipped.saturating_add(outcome.counts.skipped);
        report.over_latency_budget = report
            .over_latency_budget
            .saturating_add(outcome.counts.over_latency_budget);
        report.over_memory_budget = report
            .over_memory_budget
            .saturating_add(outcome.counts.over_memory_budget);
        report.rows.push(outcome.row);
    }
    report
}

fn run_benchmark_case(
    case: &BenchmarkCase,
    quickjs: Option<&Path>,
    engine: Option<&Path>,
) -> BenchmarkOutcome {
    let in_process = match measure_in_process(case.path, BENCH_ITERATIONS) {
        Ok(measurements) => measurements,
        Err(error) => return failed_outcome(case, &error.to_string()),
    };

    let Some(engine) = engine else {
        return failed_outcome_with_in_process(case, in_process, REASON_ENGINE_ENV_MISSING);
    };

    match measure_cli(engine, case.path, BENCH_ITERATIONS, "rsqjs") {
        Ok(ours) => benchmark_with_ours(case, quickjs, engine, ours, in_process),
        Err(error) => failed_outcome_with_in_process(case, in_process, &error.to_string()),
    }
}

fn benchmark_with_ours(
    case: &BenchmarkCase,
    quickjs: Option<&Path>,
    engine: &Path,
    ours: Duration,
    in_process: InProcessMeasurements,
) -> BenchmarkOutcome {
    let ours_memory = measure_peak_rss(engine, case.path, "rsqjs", case.id);
    let Some(quickjs) = quickjs else {
        return measured_without_reference(case, ours, in_process, &ours_memory);
    };

    match measure_cli(quickjs, case.path, BENCH_ITERATIONS, "QuickJS") {
        Ok(quickjs_duration) => {
            let quickjs_memory = measure_peak_rss(quickjs, case.path, "quickjs", case.id);
            measured_with_reference(
                case,
                ours,
                in_process,
                quickjs_duration,
                &ours_memory,
                &quickjs_memory,
            )
        }
        Err(error) => {
            failed_outcome_with_ours(case, ours, in_process, &ours_memory, &error.to_string())
        }
    }
}

fn measured_without_reference(
    case: &BenchmarkCase,
    ours: Duration,
    in_process: InProcessMeasurements,
    ours_memory: &MemoryMeasurement,
) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_MEASURED.to_owned(),
            source: case.path.to_owned(),
            iterations: BENCH_ITERATIONS,
            rsqjs_in_process_avg: format_duration(in_process.cold_eval),
            rsqjs_compile_avg: format_duration(in_process.compile),
            rsqjs_compiled_eval_avg: format_duration(in_process.compiled_eval),
            rsqjs_cli_avg: format_duration(ours),
            quickjs_cli_avg: STATUS_NOT_CONFIGURED.to_owned(),
            latency_ratio: "-".to_owned(),
            latency_budget: BUDGET_NOT_CONFIGURED.to_owned(),
            rsqjs_peak_rss: format_memory(ours_memory),
            quickjs_peak_rss: STATUS_NOT_CONFIGURED.to_owned(),
            memory_ratio: "-".to_owned(),
            memory_budget: BUDGET_NOT_CONFIGURED.to_owned(),
            detail: format_detail(&[], ours_memory, &MemoryMeasurement::NotConfigured),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            skipped: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn measured_with_reference(
    case: &BenchmarkCase,
    ours: Duration,
    in_process: InProcessMeasurements,
    quickjs: Duration,
    ours_memory: &MemoryMeasurement,
    quickjs_memory: &MemoryMeasurement,
) -> BenchmarkOutcome {
    let latency_budget = budget_check(ours.as_nanos(), quickjs.as_nanos());
    let memory_budget = memory_budget_check(ours_memory, quickjs_memory);
    let over_latency_budget = latency_budget.over_budget;
    let over_memory_budget = memory_budget.over_budget;
    let detail_flags = detail_flags(over_latency_budget, over_memory_budget);

    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: benchmark_status(over_latency_budget, over_memory_budget).to_owned(),
            source: case.path.to_owned(),
            iterations: BENCH_ITERATIONS,
            rsqjs_in_process_avg: format_duration(in_process.cold_eval),
            rsqjs_compile_avg: format_duration(in_process.compile),
            rsqjs_compiled_eval_avg: format_duration(in_process.compiled_eval),
            rsqjs_cli_avg: format_duration(ours),
            quickjs_cli_avg: format_duration(quickjs),
            latency_ratio: ratio_values(ours.as_nanos(), quickjs.as_nanos()),
            latency_budget: latency_budget.label.to_owned(),
            rsqjs_peak_rss: format_memory(ours_memory),
            quickjs_peak_rss: format_memory(quickjs_memory),
            memory_ratio: memory_ratio(ours_memory, quickjs_memory),
            memory_budget: memory_budget.label.to_owned(),
            detail: format_detail(&detail_flags, ours_memory, quickjs_memory),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            over_latency_budget: count_if(over_latency_budget),
            over_memory_budget: count_if(over_memory_budget),
            ..BenchmarkCounts::default()
        },
    }
}

fn failed_outcome(case: &BenchmarkCase, detail: &str) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            iterations: BENCH_ITERATIONS,
            rsqjs_in_process_avg: "-".to_owned(),
            rsqjs_compile_avg: "-".to_owned(),
            rsqjs_compiled_eval_avg: "-".to_owned(),
            rsqjs_cli_avg: STATUS_FAILED.to_owned(),
            quickjs_cli_avg: "-".to_owned(),
            latency_ratio: "-".to_owned(),
            latency_budget: "-".to_owned(),
            rsqjs_peak_rss: "-".to_owned(),
            quickjs_peak_rss: "-".to_owned(),
            memory_ratio: "-".to_owned(),
            memory_budget: "-".to_owned(),
            detail: detail.to_owned(),
        },
        counts: BenchmarkCounts {
            failed: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn failed_outcome_with_in_process(
    case: &BenchmarkCase,
    in_process: InProcessMeasurements,
    detail: &str,
) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            iterations: BENCH_ITERATIONS,
            rsqjs_in_process_avg: format_duration(in_process.cold_eval),
            rsqjs_compile_avg: format_duration(in_process.compile),
            rsqjs_compiled_eval_avg: format_duration(in_process.compiled_eval),
            rsqjs_cli_avg: STATUS_FAILED.to_owned(),
            quickjs_cli_avg: "-".to_owned(),
            latency_ratio: "-".to_owned(),
            latency_budget: "-".to_owned(),
            rsqjs_peak_rss: "-".to_owned(),
            quickjs_peak_rss: "-".to_owned(),
            memory_ratio: "-".to_owned(),
            memory_budget: "-".to_owned(),
            detail: detail.to_owned(),
        },
        counts: BenchmarkCounts {
            in_process_measured: 1,
            failed: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn failed_outcome_with_ours(
    case: &BenchmarkCase,
    ours: Duration,
    in_process: InProcessMeasurements,
    ours_memory: &MemoryMeasurement,
    detail: &str,
) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            iterations: BENCH_ITERATIONS,
            rsqjs_in_process_avg: format_duration(in_process.cold_eval),
            rsqjs_compile_avg: format_duration(in_process.compile),
            rsqjs_compiled_eval_avg: format_duration(in_process.compiled_eval),
            rsqjs_cli_avg: format_duration(ours),
            quickjs_cli_avg: STATUS_FAILED.to_owned(),
            latency_ratio: "-".to_owned(),
            latency_budget: "-".to_owned(),
            rsqjs_peak_rss: format_memory(ours_memory),
            quickjs_peak_rss: "-".to_owned(),
            memory_ratio: "-".to_owned(),
            memory_budget: "-".to_owned(),
            detail: detail.to_owned(),
        },
        counts: BenchmarkCounts {
            in_process_measured: 1,
            failed: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn measure_in_process(path: &str, iterations: usize) -> anyhow::Result<InProcessMeasurements> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read in-process benchmark source '{path}'"))?;
    measure_in_process_source(&source, iterations, path)
}

fn measure_in_process_source(
    source: &str,
    iterations: usize,
    label: &str,
) -> anyhow::Result<InProcessMeasurements> {
    Ok(InProcessMeasurements {
        cold_eval: measure_cold_eval_source(source, iterations, label)?,
        compile: measure_compile_source(source, iterations, label)?,
        compiled_eval: measure_compiled_eval_source(source, iterations, label)?,
    })
}

fn measure_cold_eval_source(
    source: &str,
    iterations: usize,
    label: &str,
) -> anyhow::Result<Duration> {
    let start = Instant::now();
    for _ in 0..iterations {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        let value = context
            .eval(source)
            .with_context(|| format!("in-process benchmark '{label}' failed"))?;
        black_box(value);
        black_box(context.output().len());
    }
    avg_duration(start.elapsed(), iterations)
}

fn measure_compile_source(
    source: &str,
    iterations: usize,
    label: &str,
) -> anyhow::Result<Duration> {
    let runtime = Runtime::new();
    let start = Instant::now();
    for _ in 0..iterations {
        let script = runtime
            .compile(source)
            .with_context(|| format!("compile benchmark '{label}' failed"))?;
        black_box(script.usage());
    }
    avg_duration(start.elapsed(), iterations)
}

fn measure_compiled_eval_source(
    source: &str,
    iterations: usize,
    label: &str,
) -> anyhow::Result<Duration> {
    let runtime = Runtime::new();
    let script = runtime
        .compile(source)
        .with_context(|| format!("compile step for compiled-eval benchmark '{label}' failed"))?;
    let start = Instant::now();
    for _ in 0..iterations {
        let mut context = runtime.context();
        let value = context
            .eval_compiled(&script)
            .with_context(|| format!("compiled-eval benchmark '{label}' failed"))?;
        black_box(value);
        black_box(context.output().len());
    }
    avg_duration(start.elapsed(), iterations)
}

fn measure_cli(
    engine: &Path,
    path: &str,
    iterations: usize,
    label: &str,
) -> anyhow::Result<Duration> {
    let start = Instant::now();
    for _ in 0..iterations {
        let output = Command::new(engine)
            .arg(path)
            .output()
            .with_context(|| format!("failed to execute {label} '{}'", engine.display()))?;
        if !output.status.success() {
            bail!(
                "{} benchmark '{}' failed: {}",
                label,
                path,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    avg_duration(start.elapsed(), iterations)
}

fn avg_duration(total: Duration, iterations: usize) -> anyhow::Result<Duration> {
    let divisor = u32::try_from(iterations).context("benchmark iteration count is too large")?;
    total
        .checked_div(divisor)
        .context("benchmark iteration count must be non-zero")
}

fn measure_peak_rss(engine: &Path, path: &str, label: &str, case_id: &str) -> MemoryMeasurement {
    let time_path = Path::new(GNU_TIME_PATH);
    if !time_path.is_file() {
        return MemoryMeasurement::Unavailable(format!("{GNU_TIME_PATH} is not available"));
    }

    let report_path = memory_report_path(label, case_id);
    let output = Command::new(time_path)
        .arg("-f")
        .arg("%M")
        .arg("-o")
        .arg(&report_path)
        .arg(engine)
        .arg(path)
        .output();
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return MemoryMeasurement::Unavailable(format!(
                "failed to execute GNU time for {label}: {error}"
            ));
        }
    };

    if !output.status.success() {
        let cleanup_error = cleanup_temp_file(&report_path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return memory_unavailable_with_cleanup(
            format!("GNU time failed for {label}: {stderr}"),
            cleanup_error,
        );
    }

    let rss_text = fs::read_to_string(&report_path);
    let cleanup_error = cleanup_temp_file(&report_path);
    let rss_text = match rss_text {
        Ok(text) => text,
        Err(error) => {
            return memory_unavailable_with_cleanup(
                format!(
                    "failed to read GNU time RSS report '{}': {error}",
                    report_path.display()
                ),
                cleanup_error,
            );
        }
    };

    let trimmed = rss_text.trim();
    let kib = match trimmed.parse::<u64>() {
        Ok(kib) => kib,
        Err(error) => {
            return memory_unavailable_with_cleanup(
                format!("failed to parse GNU time RSS '{trimmed}': {error}"),
                cleanup_error,
            );
        }
    };

    if let Some(error) = cleanup_error {
        return MemoryMeasurement::Unavailable(error);
    }

    MemoryMeasurement::Measured(kib)
}

fn memory_report_path(label: &str, case_id: &str) -> PathBuf {
    let file_name = format!(
        "rsqjs-bench-{}-{}-{}.rss",
        process::id(),
        sanitize_path_segment(label),
        sanitize_path_segment(case_id)
    );
    std::env::temp_dir().join(file_name)
}

fn sanitize_path_segment(input: &str) -> String {
    let segment = input
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if segment.is_empty() {
        return "value".to_owned();
    }
    segment
}

fn cleanup_temp_file(path: &Path) -> Option<String> {
    match fs::remove_file(path) {
        Ok(()) => None,
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => Some(format!(
            "failed to remove memory report '{}': {error}",
            path.display()
        )),
    }
}

fn memory_unavailable_with_cleanup(
    message: String,
    cleanup_error: Option<String>,
) -> MemoryMeasurement {
    let Some(cleanup_error) = cleanup_error else {
        return MemoryMeasurement::Unavailable(message);
    };
    MemoryMeasurement::Unavailable(format!("{message}; {cleanup_error}"))
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

fn memory_budget_check(ours: &MemoryMeasurement, quickjs: &MemoryMeasurement) -> BudgetCheck {
    let (MemoryMeasurement::Measured(ours), MemoryMeasurement::Measured(quickjs)) = (ours, quickjs)
    else {
        return BudgetCheck {
            label: BUDGET_NOT_AVAILABLE,
            over_budget: false,
        };
    };
    budget_check(u128::from(*ours), u128::from(*quickjs))
}

const fn benchmark_status(over_latency_budget: bool, over_memory_budget: bool) -> &'static str {
    if over_latency_budget || over_memory_budget {
        return STATUS_TRACKED_EXCEPTION;
    }
    STATUS_WITHIN_BUDGET
}

const fn count_if(condition: bool) -> usize {
    if condition {
        return 1;
    }
    0
}

fn detail_flags(over_latency_budget: bool, over_memory_budget: bool) -> Vec<&'static str> {
    let mut flags = Vec::new();
    if over_latency_budget {
        flags.push(DETAIL_LATENCY_EXCEPTION);
    }
    if over_memory_budget {
        flags.push(DETAIL_MEMORY_EXCEPTION);
    }
    flags
}

fn format_detail(
    flags: &[&str],
    ours_memory: &MemoryMeasurement,
    quickjs_memory: &MemoryMeasurement,
) -> String {
    let mut details = vec![DETAIL_COMPLETED.to_owned()];
    details.extend(flags.iter().map(|flag| (*flag).to_owned()));
    if let Some(note) = memory_note("rsqjs", ours_memory) {
        details.push(note);
    }
    if let Some(note) = memory_note("QuickJS", quickjs_memory) {
        details.push(note);
    }
    details.join("; ")
}

fn memory_note(label: &str, memory: &MemoryMeasurement) -> Option<String> {
    let MemoryMeasurement::Unavailable(reason) = memory else {
        return None;
    };
    Some(format!("{label} memory unavailable: {reason}"))
}

fn format_memory(memory: &MemoryMeasurement) -> String {
    match memory {
        MemoryMeasurement::Measured(kib) => format!("{kib} {MEMORY_UNIT}"),
        MemoryMeasurement::NotConfigured => STATUS_NOT_CONFIGURED.to_owned(),
        MemoryMeasurement::Unavailable(_) => STATUS_NOT_AVAILABLE.to_owned(),
    }
}

fn memory_ratio(ours: &MemoryMeasurement, quickjs: &MemoryMeasurement) -> String {
    let (MemoryMeasurement::Measured(ours), MemoryMeasurement::Measured(quickjs)) = (ours, quickjs)
    else {
        return "-".to_owned();
    };
    ratio_values(u128::from(*ours), u128::from(*quickjs))
}

fn format_duration(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    if nanos < NANOS_PER_MICROSECOND {
        return format!("{nanos} ns");
    }
    if nanos < NANOS_PER_MILLISECOND {
        return format!("{} us", nanos / NANOS_PER_MICROSECOND);
    }
    format!("{} ms", nanos / NANOS_PER_MILLISECOND)
}

fn ratio_values(ours: u128, reference: u128) -> String {
    if reference == 0 {
        return "-".to_owned();
    }
    let scaled_ratio = ours.saturating_mul(RATIO_DECIMAL_SCALE) / reference;
    format!(
        "{}.{:02}x",
        scaled_ratio / RATIO_DECIMAL_SCALE,
        scaled_ratio % RATIO_DECIMAL_SCALE
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        BUDGET_OVER, BUDGET_WITHIN, budget_check, format_duration, measure_in_process_source,
        ratio_values,
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn formats_ratio_below_one() -> TestResult {
        ensure_text(
            &ratio_values(
                Duration::from_micros(5).as_nanos(),
                Duration::from_micros(366).as_nanos(),
            ),
            "0.01x",
        )
    }

    #[test]
    fn formats_ratio_above_one() -> TestResult {
        ensure_text(
            &ratio_values(
                Duration::from_micros(250).as_nanos(),
                Duration::from_micros(100).as_nanos(),
            ),
            "2.50x",
        )
    }

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
    fn formats_millisecond_duration() -> TestResult {
        ensure_text(&format_duration(Duration::from_micros(1_500)), "1 ms")
    }

    #[test]
    fn measures_in_process_source() -> TestResult {
        let measurements = measure_in_process_source("let value = 40 + 2; value", 1, "unit-test")?;
        ensure_bool(
            measurements.cold_eval <= Duration::from_secs(1),
            "in-process cold eval should finish quickly",
        )?;
        ensure_bool(
            measurements.compile <= Duration::from_secs(1),
            "in-process compile should finish quickly",
        )?;
        ensure_bool(
            measurements.compiled_eval <= Duration::from_secs(1),
            "in-process compiled eval should finish quickly",
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
