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

mod bench_engines;
mod bench_measure;
mod benchmark_mode;
mod benchmarks;
mod build_info;
mod cases;
mod failure_classification;
mod jetstream;
#[cfg(test)]
mod report_formatting_tests;
mod report_metadata;
mod report_rollup;
mod report_text;
mod runner_cli;
mod test262_external;
mod test262_full;
mod test262_metadata;
use cases::{DifferentialCase, EngineCase, Expectation};
use runner_cli::{Config, print_rollup_outputs};

const STATUS_PASSED: &str = "✅ passed";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_SKIPPED: &str = "🟡 skipped";
const REPORT_TITLE: &str = "# rs-quickjs Test Report";
const RUNNER_NAME: &str = "`rsqjs-test-runner`";
const NO_FAILED_CASES: &str = "No failed cases.";
const TEST262_FULL_CORPUS_NAME: &str = "Test262 full corpus";
const FAILED_CASE_DETAIL_LIMIT: usize = 30;
const FEATURE_AREA_ROW_LIMIT: usize = 40;
const BASIS_POINTS_SCALE: usize = 10_000;
const PERCENT_SCALE: usize = 100;
const COVERAGE_SCALE: usize = 1_000_000;
const COVERAGE_MINOR_SCALE: usize = 10_000;
const QUICKJS_ENV: &str = "RSQJS_QUICKJS";
const TEST262_ENV: &str = "RSQJS_TEST262_DIR";
const JETSTREAM_REPORT_ENV: &str = "RSQJS_JETSTREAM_REPORT_PATH";

const REASON_MATCHED: &str = "matched expected behavior";
const REASON_QUICKJS_ENV_MISSING: &str = "set RSQJS_QUICKJS=/path/to/qjs to enable";
const OTHER_FEATURE_AREAS: &str = "other feature areas";
const NO_SKIP_REASON: &str = "none";

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let config = Config::from_args(env::args().skip(1))?;
    let report_path = match config {
        Config::Run { report_path } => report_path,
        Config::Benchmarks { report_path } => {
            return benchmark_mode::run(&report_path);
        }
        Config::AggregateReports { report_dir } => {
            let outputs = report_rollup::generate_from_report_dir(&report_dir)?;
            print_rollup_outputs(&outputs);
            return Ok(());
        }
    };

    let quickjs = env::var_os(QUICKJS_ENV).map(PathBuf::from);
    let test262 = env::var_os(TEST262_ENV).map(PathBuf::from);
    let report = build_report(quickjs.as_deref(), test262.as_deref());
    write_report(&report_path, &report)?;
    write_jetstream_report_from_env(&report)?;
    let outputs = report_rollup::generate_from_report_path(&report_path)?;
    print_rollup_outputs(&outputs);

    if report.failed_count() == 0 {
        return Ok(());
    }

    bail!(
        "test runner recorded {} failed case(s); report written to {}",
        report.failed_count(),
        report_path.display()
    )
}

fn write_jetstream_report_from_env(report: &FullReport) -> anyhow::Result<()> {
    let Some(path) = env::var_os(JETSTREAM_REPORT_ENV) else {
        return Ok(());
    };
    jetstream::write_report(Path::new(&path), &report.metadata, &report.jetstream)
}

#[derive(Debug)]
struct FullReport {
    metadata: report_metadata::RunMetadata,
    corpora: Vec<CorpusReport>,
    benchmarks: benchmarks::BenchmarkReport,
    jetstream: jetstream::JetStreamReport,
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
    feature_areas: Vec<FeatureAreaRow>,
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
            feature_areas: Vec::new(),
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

#[derive(Debug, Clone, Tabled)]
struct FeatureAreaRow {
    feature_area: String,
    total: usize,
    executed: usize,
    passed: String,
    failed: String,
    skipped: String,
    pass_rate: String,
    manifest_enabled: usize,
    top_skip_reason: String,
}

#[derive(Debug, Clone)]
struct FeatureAreaStats {
    feature_area: String,
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    manifest_enabled: usize,
    skip_reasons: BTreeMap<String, usize>,
}

impl FeatureAreaStats {
    const fn new(feature_area: String) -> Self {
        Self {
            feature_area,
            total: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
            manifest_enabled: 0,
            skip_reasons: BTreeMap::new(),
        }
    }

    const fn record_passed(&mut self) {
        self.total = self.total.saturating_add(1);
        self.passed = self.passed.saturating_add(1);
    }

    const fn record_failed(&mut self) {
        self.total = self.total.saturating_add(1);
        self.failed = self.failed.saturating_add(1);
    }

    fn record_skipped(&mut self, reason: String) {
        self.total = self.total.saturating_add(1);
        self.skipped = self.skipped.saturating_add(1);
        let count = self.skip_reasons.entry(reason).or_default();
        *count = count.saturating_add(1);
    }

    const fn record_manifest_enabled(&mut self) {
        self.manifest_enabled = self.manifest_enabled.saturating_add(1);
    }

    const fn executed(&self) -> usize {
        self.passed.saturating_add(self.failed)
    }

    fn merge(&mut self, other: Self) {
        self.total = self.total.saturating_add(other.total);
        self.passed = self.passed.saturating_add(other.passed);
        self.failed = self.failed.saturating_add(other.failed);
        self.skipped = self.skipped.saturating_add(other.skipped);
        self.manifest_enabled = self.manifest_enabled.saturating_add(other.manifest_enabled);
        for (reason, skipped) in other.skip_reasons {
            let count = self.skip_reasons.entry(reason).or_default();
            *count = count.saturating_add(skipped);
        }
    }

    fn top_skip_reason(&self) -> String {
        let mut best = None::<(&String, usize)>;
        for (reason, skipped) in &self.skip_reasons {
            let skipped = *skipped;
            let replace = match best {
                None => true,
                Some((best_reason, best_skipped)) => {
                    skipped > best_skipped || skipped == best_skipped && reason < best_reason
                }
            };
            if replace {
                best = Some((reason, skipped));
            }
        }
        if let Some((reason, skipped)) = best {
            return format!("{skipped}: {reason}");
        }
        NO_SKIP_REASON.to_owned()
    }
}

fn build_report(quickjs: Option<&Path>, test262: Option<&Path>) -> FullReport {
    let mut corpora = vec![run_engine_corpus(), run_test262_corpus()];
    corpora.extend(test262_full::run_reports(test262));
    corpora.push(run_quickjs_corpus(quickjs));
    let benchmarks = benchmarks::run();
    let jetstream = jetstream::run();
    FullReport {
        metadata: report_metadata::RunMetadata::from_env(),
        corpora,
        benchmarks,
        jetstream,
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
            detail: report_text::table_detail(&error.to_string()),
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
            detail: report_text::table_detail(&error.to_string()),
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
    ];
    sections.extend(report_metadata::render_section(&report.metadata));
    sections.extend([
        "Corpus detail sections list failed cases only. Passed and skipped cases are summarized to keep the report compact.".to_owned(),
        "The full Test262 corpus is progress-only; the active subset remains the CI gate.".to_owned(),
        "Test262 file conformance collapses required variants by source file for dashboard comparison.".to_owned(),
        "Test262 full corpus keeps default, strict, module, and raw variants as diagnostic rows.".to_owned(),
        String::new(),
        "## Corpus Summary".to_owned(),
        String::new(),
        fenced_table(&Table::new(corpus_summary_rows(report))),
    ]);
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
        if !corpus.feature_areas.is_empty() {
            sections.push(String::new());
            sections.push("### Feature Map".to_owned());
            sections.push(String::new());
            sections.push(
                "Feature areas aggregate Test262 variants by source path. Passed and skipped cases are summarized, not listed."
                    .to_owned(),
            );
            sections.push(String::new());
            sections.push(fenced_table(&Table::new(&corpus.feature_areas)));
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
        "- Measured: {}\n- In-process measured: {}\n- Failed: {}\n- Invalid: {}\n- Skipped reference: {}\n- Over latency budget ({}): {}\n- Over memory budget ({}): {}",
        report.benchmarks.measured,
        report.benchmarks.in_process_measured,
        report.benchmarks.failed,
        report.benchmarks.invalid,
        report.benchmarks.skipped,
        benchmarks::BUDGET_LABEL,
        report.benchmarks.over_latency_budget,
        benchmarks::BUDGET_LABEL,
        report.benchmarks.over_memory_budget,
    ));
    sections.push(String::new());
    sections.push(fenced_table(&Table::new(&report.benchmarks.rows)));
    sections.push(String::new());
    sections.extend(jetstream::render_section(&report.jetstream));
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
        .map(|(reason, skipped)| SkipReasonRow {
            skipped,
            reason: report_text::table_detail(&reason),
        })
        .collect()
}

fn feature_area_rows(stats: Vec<FeatureAreaStats>) -> Vec<FeatureAreaRow> {
    feature_area_rows_with_limit(stats, FEATURE_AREA_ROW_LIMIT)
}

fn feature_area_rows_with_limit(
    mut stats: Vec<FeatureAreaStats>,
    limit: usize,
) -> Vec<FeatureAreaRow> {
    stats.sort_by(|left, right| {
        right
            .total
            .cmp(&left.total)
            .then_with(|| right.failed.cmp(&left.failed))
            .then_with(|| left.feature_area.cmp(&right.feature_area))
    });

    let mut rows = Vec::new();
    let mut remainder = FeatureAreaStats::new(OTHER_FEATURE_AREAS.to_owned());
    for (index, area) in stats.into_iter().enumerate() {
        if index < limit {
            rows.push(feature_area_row(&area));
        } else {
            remainder.merge(area);
        }
    }
    if remainder.total > 0 {
        rows.push(feature_area_row(&remainder));
    }
    rows
}

fn feature_area_row(stats: &FeatureAreaStats) -> FeatureAreaRow {
    let executed = stats.executed();
    FeatureAreaRow {
        feature_area: stats.feature_area.clone(),
        total: stats.total,
        executed,
        passed: format!("{} {}", stats.passed, STATUS_PASSED),
        failed: format!("{} {}", stats.failed, STATUS_FAILED),
        skipped: format!("{} {}", stats.skipped, STATUS_SKIPPED),
        pass_rate: percent(stats.passed, executed),
        manifest_enabled: stats.manifest_enabled,
        top_skip_reason: stats.top_skip_reason(),
    }
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
