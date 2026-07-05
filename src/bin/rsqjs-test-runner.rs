use std::{
    collections::BTreeMap,
    env, fmt, fs,
    path::{Path, PathBuf},
    process,
    process::Command,
};

use anyhow::{Context as _, bail};
use rs_quickjs::{Runtime, Value};
use tabled::{Table, Tabled};

#[path = "rsqjs_test_runner/benchmarks.rs"]
mod benchmarks;
#[path = "rsqjs_test_runner/cases.rs"]
mod cases;
#[path = "rsqjs_test_runner/failure_classification.rs"]
mod failure_classification;
#[cfg(test)]
#[path = "rsqjs_test_runner/report_formatting_tests.rs"]
mod report_formatting_tests;
#[path = "rsqjs_test_runner/test262_external.rs"]
mod test262_external;
#[path = "rsqjs_test_runner/test262_full.rs"]
mod test262_full;
#[path = "rsqjs_test_runner/test262_metadata.rs"]
mod test262_metadata;

use cases::{DifferentialCase, EngineCase, Expectation};

const USAGE: &str = "usage: rsqjs-test-runner --report <path>";
const STATUS_PASSED: &str = "✅ passed";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_SKIPPED: &str = "🟡 skipped";
const REPORT_TITLE: &str = "# rs-quickjs Test Report";
const RUNNER_NAME: &str = "`rsqjs-test-runner`";
const NO_FAILED_CASES: &str = "No failed cases.";
const TEST262_FULL_CORPUS_NAME: &str = "Test262 full corpus";
const FAILED_CASE_DETAIL_LIMIT: usize = 30;
const BASIS_POINTS_SCALE: usize = 10_000;
const PERCENT_SCALE: usize = 100;
const COVERAGE_SCALE: usize = 1_000_000;
const COVERAGE_MINOR_SCALE: usize = 10_000;
const QUICKJS_ENV: &str = "RSQJS_QUICKJS";
const ENGINE_ENV: &str = "RSQJS_ENGINE";
const TEST262_ENV: &str = "RSQJS_TEST262_DIR";

const REASON_MATCHED: &str = "matched expected behavior";
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
    benchmarks: benchmarks::BenchmarkReport,
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
    let benchmarks = benchmarks::run(quickjs, engine);
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
        let failed_rows = corpus.failed_rows();
        if corpus.name == TEST262_FULL_CORPUS_NAME {
            sections.extend(failure_classification::sections(&failed_rows));
        }
        sections.push("### Failed Cases".to_owned());
        sections.push(String::new());
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
        "- Measured: {}\n- Failed: {}\n- Skipped reference: {}\n- Over latency budget ({}): {}\n- Over memory budget ({}): {}",
        report.benchmarks.measured,
        report.benchmarks.failed,
        report.benchmarks.skipped,
        benchmarks::BUDGET_LABEL,
        report.benchmarks.over_latency_budget,
        benchmarks::BUDGET_LABEL,
        report.benchmarks.over_memory_budget,
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
