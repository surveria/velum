use std::{
    collections::BTreeMap,
    env, fmt, fs,
    path::{Path, PathBuf},
    process,
    process::Command,
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail};
use rs_quickjs::{Runtime, Value};
use tabled::{Table, Tabled};

#[path = "rsqjs_test_runner/cases.rs"]
mod cases;
#[path = "rsqjs_test_runner/test262_external.rs"]
mod test262_external;
#[path = "rsqjs_test_runner/test262_full.rs"]
mod test262_full;
#[path = "rsqjs_test_runner/test262_metadata.rs"]
mod test262_metadata;

use cases::{BenchmarkCase, DifferentialCase, EngineCase, Expectation};

const USAGE: &str = "usage: rsqjs-test-runner --report <path>";
const STATUS_PASSED: &str = "✅ passed";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_SKIPPED: &str = "🟡 skipped";
const STATUS_MEASURED: &str = "✅ measured";
const STATUS_NOT_CONFIGURED: &str = "🟡 not configured";
const REPORT_TITLE: &str = "# rs-quickjs Test Report";
const RUNNER_NAME: &str = "`rsqjs-test-runner`";
const NO_FAILED_CASES: &str = "No failed cases.";
const FAILED_CASE_DETAIL_LIMIT: usize = 30;
const BASIS_POINTS_SCALE: usize = 10_000;
const PERCENT_SCALE: usize = 100;
const COVERAGE_SCALE: usize = 1_000_000;
const COVERAGE_MINOR_SCALE: usize = 10_000;
const BENCH_ITERATIONS: usize = 50;
const NANOS_PER_MICROSECOND: u128 = 1_000;
const NANOS_PER_MILLISECOND: u128 = 1_000_000;
const RATIO_DECIMAL_SCALE: u128 = 100;
const QUICKJS_ENV: &str = "RSQJS_QUICKJS";
const ENGINE_ENV: &str = "RSQJS_ENGINE";
const TEST262_ENV: &str = "RSQJS_TEST262_DIR";

const REASON_MATCHED: &str = "matched expected behavior";
const REASON_ENGINE_ENV_MISSING: &str = "set RSQJS_ENGINE=/path/to/rsqjs to enable benchmarks";
const REASON_QUICKJS_ENV_MISSING: &str = "set RSQJS_QUICKJS=/path/to/qjs to enable";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let config = Config::from_args(env::args().skip(1))?;
    let quickjs = env::var_os(QUICKJS_ENV).map(PathBuf::from);
    let engine = env::var_os(ENGINE_ENV).map(PathBuf::from);
    let test262 = env::var_os(TEST262_ENV).map(PathBuf::from);
    let report = build_report(quickjs.as_deref(), engine.as_deref(), test262.as_deref());
    write_report(&config.report_path, &report)?;

    if report.failed_count() == 0 {
        return Ok(());
    }

    bail!(
        "test runner recorded {} failed case(s); report written to {}",
        report.failed_count(),
        config.report_path.display()
    )
}

#[derive(Debug)]
struct Config {
    report_path: PathBuf,
}

impl Config {
    fn from_args(mut args: impl Iterator<Item = String>) -> anyhow::Result<Self> {
        let Some(flag) = args.next() else {
            bail!("{USAGE}");
        };
        if flag != "--report" {
            bail!("unknown argument '{flag}'; {USAGE}");
        }

        let report_path = args.next().context("missing path after --report")?;
        if let Some(extra) = args.next() {
            bail!("unexpected argument '{extra}'; {USAGE}");
        }

        Ok(Self {
            report_path: PathBuf::from(report_path),
        })
    }
}

#[derive(Debug)]
struct FullReport {
    corpora: Vec<CorpusReport>,
    benchmarks: BenchmarkReport,
}

impl FullReport {
    fn failed_count(&self) -> usize {
        let corpus_failures = self
            .corpora
            .iter()
            .filter(|corpus| corpus.required)
            .map(CorpusReport::failed)
            .fold(0usize, usize::saturating_add);
        corpus_failures.saturating_add(self.benchmarks.failed)
    }
}

#[derive(Debug)]
struct CorpusReport {
    name: &'static str,
    required: bool,
    stats: CorpusStats,
    rows: Vec<CaseRow>,
    skip_reasons: Vec<SkipReasonRow>,
}

impl CorpusReport {
    fn from_rows(name: &'static str, rows: Vec<CaseRow>) -> Self {
        let stats = CorpusStats::from_rows(&rows);
        let skip_reasons = skip_reason_rows(&rows);
        Self {
            name,
            required: true,
            stats,
            rows,
            skip_reasons,
        }
    }

    const fn total(&self) -> usize {
        self.stats.total
    }

    const fn executed(&self) -> usize {
        self.stats.executed()
    }

    const fn passed(&self) -> usize {
        self.stats.passed
    }

    const fn failed(&self) -> usize {
        self.stats.failed
    }

    const fn skipped(&self) -> usize {
        self.stats.skipped
    }

    fn failed_rows(&self) -> Vec<CaseRow> {
        self.rows
            .iter()
            .filter(|row| row.status == STATUS_FAILED)
            .cloned()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct CorpusStats {
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
}

impl CorpusStats {
    fn from_rows(rows: &[CaseRow]) -> Self {
        let total = rows.len();
        let passed = rows
            .iter()
            .filter(|row| row.status == STATUS_PASSED)
            .count();
        let failed = rows
            .iter()
            .filter(|row| row.status == STATUS_FAILED)
            .count();
        let skipped = rows
            .iter()
            .filter(|row| row.status == STATUS_SKIPPED)
            .count();
        Self {
            total,
            passed,
            failed,
            skipped,
        }
    }

    const fn executed(self) -> usize {
        self.passed.saturating_add(self.failed)
    }
}

#[derive(Debug, Tabled)]
struct CorpusSummaryRow {
    corpus: String,
    total: usize,
    executed: usize,
    passed: String,
    failed: String,
    skipped: String,
    coverage: String,
    pass_rate: String,
}

#[derive(Debug, Clone, Tabled)]
struct CaseRow {
    case: String,
    status: String,
    source: String,
    detail: String,
}

#[derive(Debug, Clone, Tabled)]
struct SkipReasonRow {
    skipped: usize,
    reason: String,
}

#[derive(Debug)]
struct BenchmarkReport {
    rows: Vec<BenchmarkRow>,
    measured: usize,
    failed: usize,
    skipped: usize,
}

#[derive(Debug, Tabled)]
struct BenchmarkRow {
    benchmark: String,
    status: String,
    source: String,
    iterations: usize,
    rsqjs_cli_avg: String,
    quickjs_cli_avg: String,
    ratio: String,
    detail: String,
}

fn build_report(
    quickjs: Option<&Path>,
    engine: Option<&Path>,
    test262: Option<&Path>,
) -> FullReport {
    let corpora = vec![
        run_engine_corpus(),
        run_test262_corpus(),
        run_test262_full_corpus(test262),
        run_quickjs_corpus(quickjs),
    ];
    let benchmarks = run_benchmarks(quickjs, engine);
    FullReport {
        corpora,
        benchmarks,
    }
}

fn run_engine_corpus() -> CorpusReport {
    let cases = cases::engine_cases();
    let rows = cases.iter().map(run_engine_case).collect();
    CorpusReport::from_rows("Engine fixtures", rows)
}

fn run_test262_corpus() -> CorpusReport {
    let cases = cases::test262_cases();
    let rows = cases.iter().map(run_engine_case).collect();
    CorpusReport::from_rows("Test262 active subset", rows)
}

fn run_test262_full_corpus(test262: Option<&Path>) -> CorpusReport {
    test262_full::run(test262)
}

fn run_quickjs_corpus(quickjs: Option<&Path>) -> CorpusReport {
    let rows = cases::quickjs_differential_cases()
        .into_iter()
        .map(|case| run_differential_case(&case, quickjs))
        .collect();
    CorpusReport::from_rows("QuickJS differential", rows)
}

fn run_engine_case(case: &EngineCase) -> CaseRow {
    match execute_engine_case(case) {
        Ok(()) => CaseRow {
            case: case.id.to_owned(),
            status: STATUS_PASSED.to_owned(),
            source: case.path.to_owned(),
            detail: REASON_MATCHED.to_owned(),
        },
        Err(error) => CaseRow {
            case: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            detail: error.to_string(),
        },
    }
}

fn execute_engine_case(case: &EngineCase) -> anyhow::Result<()> {
    let source = fs::read_to_string(case.path)
        .with_context(|| format!("failed to read test source '{}'", case.path))?;

    let runtime = Runtime::new();
    let mut context = runtime.context();
    let result = context.eval(&source);

    match &case.expectation {
        Expectation::Value(expected) => {
            let value = result
                .with_context(|| format!("case '{}' failed while evaluating source", case.id))?;
            ensure_value(case.id, &value, expected)?;
            ensure_output(case.id, context.output(), &[])?;
        }
        Expectation::OutputAndValue { output, value } => {
            let actual = result
                .with_context(|| format!("case '{}' failed while evaluating source", case.id))?;
            ensure_value(case.id, &actual, value)?;
            ensure_output(case.id, context.output(), output)?;
        }
        Expectation::ErrorContains(expected) => {
            let Err(error) = result else {
                bail!("case '{}' expected error containing '{expected}'", case.id);
            };
            let message = error.to_string();
            if !message.contains(expected) {
                bail!(
                    "case '{}' expected error containing '{}', got '{}'",
                    case.id,
                    expected,
                    message
                );
            }
        }
    }

    Ok(())
}

fn run_differential_case(case: &DifferentialCase, quickjs: Option<&Path>) -> CaseRow {
    let Some(quickjs) = quickjs else {
        return CaseRow {
            case: case.id.to_owned(),
            status: STATUS_SKIPPED.to_owned(),
            source: case.path.to_owned(),
            detail: REASON_QUICKJS_ENV_MISSING.to_owned(),
        };
    };

    match execute_differential_case(case, quickjs) {
        Ok(()) => CaseRow {
            case: case.id.to_owned(),
            status: STATUS_PASSED.to_owned(),
            source: case.path.to_owned(),
            detail: "rs-quickjs stdout matched QuickJS stdout".to_owned(),
        },
        Err(error) => CaseRow {
            case: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            detail: error.to_string(),
        },
    }
}

fn execute_differential_case(case: &DifferentialCase, quickjs: &Path) -> anyhow::Result<()> {
    let source = fs::read_to_string(case.path)
        .with_context(|| format!("failed to read differential source '{}'", case.path))?;
    let ours = run_source_with_output(&source)?;
    let quickjs_output = Command::new(quickjs)
        .arg(case.path)
        .output()
        .with_context(|| format!("failed to execute QuickJS '{}'", quickjs.display()))?;
    if !quickjs_output.status.success() {
        bail!(
            "QuickJS failed for '{}': {}",
            case.path,
            String::from_utf8_lossy(&quickjs_output.stderr)
        );
    }
    let quickjs_stdout = String::from_utf8_lossy(&quickjs_output.stdout);
    if ours != quickjs_stdout {
        bail!(
            "stdout mismatch: rs-quickjs {}, QuickJS {}",
            DisplayText(&ours),
            DisplayText(&quickjs_stdout)
        );
    }
    Ok(())
}

fn run_source_with_output(source: &str) -> anyhow::Result<String> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context
        .eval(source)
        .context("rs-quickjs evaluation failed")?;
    let mut output = context.take_output().join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    if value != Value::Undefined {
        output.push_str(&value.to_string());
        output.push('\n');
    }
    Ok(output)
}

fn run_benchmarks(quickjs: Option<&Path>, engine: Option<&Path>) -> BenchmarkReport {
    let mut report = BenchmarkReport {
        rows: Vec::new(),
        measured: 0,
        failed: 0,
        skipped: 0,
    };
    for case in cases::benchmark_cases() {
        let row = run_benchmark_case(&case, quickjs, engine);
        if row.status == STATUS_FAILED {
            report.failed = report.failed.saturating_add(1);
        } else {
            report.measured = report.measured.saturating_add(1);
            if row.quickjs_cli_avg == STATUS_NOT_CONFIGURED {
                report.skipped = report.skipped.saturating_add(1);
            }
        }
        report.rows.push(row);
    }
    report
}

fn run_benchmark_case(
    case: &BenchmarkCase,
    quickjs: Option<&Path>,
    engine: Option<&Path>,
) -> BenchmarkRow {
    let Some(engine) = engine else {
        return BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            iterations: BENCH_ITERATIONS,
            rsqjs_cli_avg: STATUS_FAILED.to_owned(),
            quickjs_cli_avg: "-".to_owned(),
            ratio: "-".to_owned(),
            detail: REASON_ENGINE_ENV_MISSING.to_owned(),
        };
    };

    match measure_cli(engine, case.path, BENCH_ITERATIONS, "rsqjs") {
        Ok(ours) => {
            let quickjs_measurement = quickjs
                .map(|quickjs| measure_cli(quickjs, case.path, BENCH_ITERATIONS, "QuickJS"))
                .transpose();
            match quickjs_measurement {
                Ok(Some(quickjs_duration)) => BenchmarkRow {
                    benchmark: case.id.to_owned(),
                    status: STATUS_MEASURED.to_owned(),
                    source: case.path.to_owned(),
                    iterations: BENCH_ITERATIONS,
                    rsqjs_cli_avg: format_duration(ours),
                    quickjs_cli_avg: format_duration(quickjs_duration),
                    ratio: ratio(ours, quickjs_duration),
                    detail: "sequential benchmark completed".to_owned(),
                },
                Ok(None) => BenchmarkRow {
                    benchmark: case.id.to_owned(),
                    status: STATUS_MEASURED.to_owned(),
                    source: case.path.to_owned(),
                    iterations: BENCH_ITERATIONS,
                    rsqjs_cli_avg: format_duration(ours),
                    quickjs_cli_avg: STATUS_NOT_CONFIGURED.to_owned(),
                    ratio: "-".to_owned(),
                    detail: REASON_QUICKJS_ENV_MISSING.to_owned(),
                },
                Err(error) => BenchmarkRow {
                    benchmark: case.id.to_owned(),
                    status: STATUS_FAILED.to_owned(),
                    source: case.path.to_owned(),
                    iterations: BENCH_ITERATIONS,
                    rsqjs_cli_avg: format_duration(ours),
                    quickjs_cli_avg: STATUS_FAILED.to_owned(),
                    ratio: "-".to_owned(),
                    detail: error.to_string(),
                },
            }
        }
        Err(error) => BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            iterations: BENCH_ITERATIONS,
            rsqjs_cli_avg: STATUS_FAILED.to_owned(),
            quickjs_cli_avg: "-".to_owned(),
            ratio: "-".to_owned(),
            detail: error.to_string(),
        },
    }
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

fn ensure_value(case_id: &str, actual: &Value, expected: &str) -> anyhow::Result<()> {
    let actual_text = actual.to_string();
    if actual_text == expected {
        return Ok(());
    }
    bail!("case '{case_id}' expected value '{expected}', got '{actual_text}'")
}

fn ensure_output(case_id: &str, actual: &[String], expected: &[&str]) -> anyhow::Result<()> {
    if actual.len() != expected.len() {
        bail!(
            "case '{}' expected output {}, got {}",
            case_id,
            DisplaySlice(expected),
            DisplaySlice(actual)
        );
    }
    for (actual_line, expected_line) in actual.iter().zip(expected.iter()) {
        if actual_line != expected_line {
            bail!(
                "case '{}' expected output {}, got {}",
                case_id,
                DisplaySlice(expected),
                DisplaySlice(actual)
            );
        }
    }
    Ok(())
}

fn write_report(path: &Path, report: &FullReport) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create report directory '{}'", parent.display()))?;
    }

    let body = render_report(report);
    fs::write(path, body)
        .with_context(|| format!("failed to write test report '{}'", path.display()))
}

fn render_report(report: &FullReport) -> String {
    let mut sections = vec![
        REPORT_TITLE.to_owned(),
        String::new(),
        format!("Generated by {RUNNER_NAME}."),
        String::new(),
        "Corpus detail sections list failed cases only. Passed and skipped cases are summarized to keep the report compact.".to_owned(),
        "The full Test262 corpus is progress-only; the active subset remains the CI gate.".to_owned(),
        "The full Test262 corpus expands upstream files into metadata-driven default, strict, or raw variants.".to_owned(),
        String::new(),
        "## Corpus Summary".to_owned(),
        String::new(),
        fenced_table(&Table::new(corpus_summary_rows(report))),
    ];
    for corpus in &report.corpora {
        sections.push(format!("## {}", corpus.name));
        sections.push(String::new());
        sections.push(format!(
            "- Total: {}\n- Executed: {}\n- Passed: {}\n- Failed: {}\n- Skipped: {}\n- Coverage: {}\n- Pass rate: {}",
            corpus.total(),
            corpus.executed(),
            corpus.passed(),
            corpus.failed(),
            corpus.skipped(),
            coverage_percent(corpus.executed(), corpus.total()),
            percent(corpus.passed(), corpus.executed()),
        ));
        if !corpus.skip_reasons.is_empty() {
            sections.push(String::new());
            sections.push("### Skip Reasons".to_owned());
            sections.push(String::new());
            sections.push(fenced_table(&Table::new(&corpus.skip_reasons)));
        }
        sections.push(String::new());
        sections.push("### Failed Cases".to_owned());
        sections.push(String::new());
        let failed_rows = corpus.failed_rows();
        if failed_rows.is_empty() {
            sections.push(NO_FAILED_CASES.to_owned());
        } else {
            let displayed_failed_rows = last_failed_rows(&failed_rows, FAILED_CASE_DETAIL_LIMIT);
            if displayed_failed_rows.len() < failed_rows.len() {
                sections.push(format!(
                    "Showing the last {} of {} failed cases.",
                    displayed_failed_rows.len(),
                    failed_rows.len()
                ));
                sections.push(String::new());
            }
            sections.push(fenced_table(&Table::new(&displayed_failed_rows)));
        }
    }
    sections.push("## Benchmarks".to_owned());
    sections.push(String::new());
    sections.push(format!(
        "- Measured: {}\n- Failed: {}\n- Skipped reference: {}",
        report.benchmarks.measured, report.benchmarks.failed, report.benchmarks.skipped
    ));
    sections.push(String::new());
    sections.push(fenced_table(&Table::new(&report.benchmarks.rows)));
    sections.push(String::new());
    sections.join("\n")
}

fn last_failed_rows(rows: &[CaseRow], limit: usize) -> Vec<CaseRow> {
    let skip = rows.len().saturating_sub(limit);
    rows.iter().skip(skip).cloned().collect()
}

fn corpus_summary_rows(report: &FullReport) -> Vec<CorpusSummaryRow> {
    report
        .corpora
        .iter()
        .map(|corpus| CorpusSummaryRow {
            corpus: corpus.name.to_owned(),
            total: corpus.total(),
            executed: corpus.executed(),
            passed: format!("{} {}", corpus.passed(), STATUS_PASSED),
            failed: format!("{} {}", corpus.failed(), STATUS_FAILED),
            skipped: format!("{} {}", corpus.skipped(), STATUS_SKIPPED),
            coverage: coverage_percent(corpus.executed(), corpus.total()),
            pass_rate: percent(corpus.passed(), corpus.executed()),
        })
        .collect()
}

fn skip_reason_rows(rows: &[CaseRow]) -> Vec<SkipReasonRow> {
    let mut reasons = BTreeMap::<String, usize>::new();
    for row in rows {
        if row.status == STATUS_SKIPPED {
            let count = reasons.entry(row.detail.clone()).or_default();
            *count = count.saturating_add(1);
        }
    }
    reasons
        .into_iter()
        .map(|(reason, skipped)| SkipReasonRow { skipped, reason })
        .collect()
}

fn fenced_table(table: &Table) -> String {
    format!("```text\n{table}\n```")
}

fn percent(part: usize, total: usize) -> String {
    if total == 0 {
        return "0.00%".to_owned();
    }
    let basis_points = part.saturating_mul(BASIS_POINTS_SCALE) / total;
    let major = basis_points / PERCENT_SCALE;
    let minor = basis_points % PERCENT_SCALE;
    format!("{major}.{minor:02}%")
}

fn coverage_percent(part: usize, total: usize) -> String {
    if total == 0 {
        return "0.0000%".to_owned();
    }
    let scaled = part.saturating_mul(COVERAGE_SCALE) / total;
    let major = scaled / COVERAGE_MINOR_SCALE;
    let minor = scaled % COVERAGE_MINOR_SCALE;
    format!("{major}.{minor:04}%")
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

fn ratio(ours: Duration, quickjs: Duration) -> String {
    let quickjs_nanos = quickjs.as_nanos();
    if quickjs_nanos == 0 {
        return "-".to_owned();
    }
    let scaled_ratio = ours.as_nanos().saturating_mul(RATIO_DECIMAL_SCALE) / quickjs_nanos;
    format!(
        "{}.{:02}x",
        scaled_ratio / RATIO_DECIMAL_SCALE,
        scaled_ratio % RATIO_DECIMAL_SCALE
    )
}

struct DisplaySlice<'a, T>(&'a [T]);

impl<T: fmt::Display> fmt::Display for DisplaySlice<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[")?;
        let mut first = true;
        for item in self.0 {
            if first {
                first = false;
            } else {
                formatter.write_str(", ")?;
            }
            write!(formatter, "\"{item}\"")?;
        }
        formatter.write_str("]")
    }
}

struct DisplayText<'a>(&'a str);

impl fmt::Display for DisplayText<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{coverage_percent, ratio};
    use std::time::Duration;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn formats_ratio_below_one() -> TestResult {
        ensure_text(
            &ratio(Duration::from_micros(5), Duration::from_micros(366)),
            "0.01x",
        )
    }

    #[test]
    fn formats_ratio_above_one() -> TestResult {
        ensure_text(
            &ratio(Duration::from_micros(250), Duration::from_micros(100)),
            "2.50x",
        )
    }

    #[test]
    fn formats_small_coverage_with_four_decimals() -> TestResult {
        ensure_text(&coverage_percent(4, 53_683), "0.0074%")
    }

    fn ensure_text(actual: &str, expected: &str) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected '{expected}', got '{actual}'").into())
    }
}
