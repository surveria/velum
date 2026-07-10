use std::{
    collections::BTreeMap,
    env, fmt, fs,
    path::{Path, PathBuf},
    process,
    process::Command,
    time::Duration,
};

use anyhow::{Context as _, bail};
use rs_quickjs::{Runtime, Value};
use tabled::Tabled;

mod bench_engines;
mod bench_measure;
mod benchmark_case;
mod benchmark_mode;
mod benchmark_protocol;
mod benchmark_selection;
mod benchmarks;
mod build_info;
mod case_registry;
mod cases;
mod failure_classification;
mod jetstream;
mod jetstream_baseline;
mod jetstream_mode;
mod prepared_benchmarks;
mod quickjs_baseline;
mod report_benchmark_methodology;
mod report_composition;
#[cfg(test)]
mod report_formatting_tests;
mod report_metadata;
mod report_methodology_rendering;
mod report_rendering;
mod report_rollup;
mod report_schema;
mod report_schema_io;
mod report_schema_support;
#[cfg(test)]
mod report_schema_tests;
mod report_schema_validation;
mod report_text;
mod runner_cli;
mod test262_baseline;
mod test262_external;
mod test262_full;
mod test262_metadata;
mod test262_parallel;
mod timing;
use cases::{DifferentialCase, EngineCase, Expectation};
#[cfg(test)]
use report_rendering::{coverage_percent, feature_area_rows_with_limit};
use report_rendering::{
    feature_area_rows, fenced_table, render_report, render_timing_tsv, skip_reason_rows,
};
use report_schema::{EnvironmentInfo, ReportDocument, ReportMode, RunConfiguration};
use runner_cli::{Config, print_rollup_outputs};

const STATUS_PASSED: &str = "✅ passed";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_SKIPPED: &str = "🟡 skipped";
const REPORT_TITLE: &str = "# rs-quickjs Test Report";
const RUNNER_NAME: &str = "`rsqjs-test-runner`";
const QUICKJS_ENV: &str = "RSQJS_QUICKJS";
const TEST262_ENV: &str = "RSQJS_TEST262_DIR";
const JETSTREAM_REPORT_ENV: &str = "RSQJS_JETSTREAM_REPORT_PATH";
const JETSTREAM_ENABLED_ENV: &str = "RSQJS_JETSTREAM_ENABLED";

const REASON_MATCHED: &str = "matched expected behavior";
const REASON_QUICKJS_ENV_MISSING: &str = "set RSQJS_QUICKJS=/path/to/qjs to enable";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ReportKind {
    Full,
    Correctness,
    Performance,
}

impl ReportKind {
    const fn schema_mode(self) -> ReportMode {
        match self {
            Self::Full => ReportMode::Full,
            Self::Correctness => ReportMode::Correctness,
            Self::Performance => ReportMode::Performance,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let config = Config::from_args(env::args().skip(1))?;
    let (report_path, report_kind) = match config {
        Config::Run { report_path } => (report_path, ReportKind::Full),
        Config::Correctness { report_path } => (report_path, ReportKind::Correctness),
        Config::Performance { report_path } => (report_path, ReportKind::Performance),
        Config::Benchmarks { report_path } => {
            return benchmark_mode::run(&report_path);
        }
        Config::ComposeReports {
            expected_tree,
            correctness_path,
            performance_path,
            report_path,
        } => {
            let correctness = report_schema_io::read_document(&correctness_path)?;
            let performance = report_schema_io::read_document(&performance_path)?;
            let report = report_composition::compose(correctness, performance, &expected_tree)?;
            report_composition::validate_output_path(&report, &report_path)?;
            return write_report(&report_path, &report);
        }
        Config::JetStream { report_path } => {
            return jetstream_mode::run(&report_path);
        }
        Config::AggregateReports { report_dir } => {
            let outputs = report_rollup::generate_from_report_dir(&report_dir)?;
            print_rollup_outputs(&outputs);
            return Ok(());
        }
    };

    case_registry::validate()?;
    let quickjs = env::var_os(QUICKJS_ENV).map(PathBuf::from);
    let test262 = env::var_os(TEST262_ENV).map(PathBuf::from);
    let include_jetstream = report_kind == ReportKind::Full && jetstream_enabled();
    let environment = EnvironmentInfo::capture();
    let run_configuration = RunConfiguration::capture(
        quickjs.is_some(),
        test262.is_some(),
        report_kind.schema_mode(),
        include_jetstream,
    );
    let report = build_report(
        quickjs.as_deref(),
        test262.as_deref(),
        report_kind,
        include_jetstream,
    )?;
    if include_jetstream {
        write_jetstream_report_from_env(&report)?;
    }
    let report = ReportDocument::from_run(report, environment, run_configuration)?;
    write_report(&report_path, &report)?;
    if report_kind == ReportKind::Full {
        let outputs = report_rollup::generate_from_report_path(&report_path)?;
        print_rollup_outputs(&outputs);
    }

    if report.failed_count() == 0 {
        return Ok(());
    }

    bail!(
        "test runner recorded {} failed case(s); report written to {}",
        report.failed_count(),
        report_path.display()
    )
}

fn jetstream_enabled() -> bool {
    env::var(JETSTREAM_ENABLED_ENV).map_or(true, |value| {
        let value = value.trim();
        value != "0" && !value.eq_ignore_ascii_case("false") && !value.eq_ignore_ascii_case("no")
    })
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
    elapsed: Duration,
}

#[derive(Debug)]
struct CorpusReport {
    name: &'static str,
    required: bool,
    stats: CorpusStats,
    rows: Vec<CaseRow>,
    skip_reasons: Vec<SkipReasonRow>,
    feature_areas: Vec<FeatureAreaRow>,
    elapsed: Duration,
}

impl CorpusReport {
    fn from_rows(name: &'static str, rows: Vec<CaseRow>, elapsed: Duration) -> Self {
        let stats = CorpusStats::from_rows(&rows);
        let skip_reasons = skip_reason_rows(&rows);
        Self {
            name,
            required: true,
            stats,
            rows,
            skip_reasons,
            feature_areas: Vec::new(),
            elapsed,
        }
    }

    const fn total(&self) -> usize {
        self.stats.total
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
}

#[derive(Debug, Clone)]
struct CaseRow {
    case: String,
    status: String,
    source: String,
    detail: String,
    elapsed: Duration,
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
}

fn build_report(
    quickjs: Option<&Path>,
    test262: Option<&Path>,
    report_kind: ReportKind,
    include_jetstream: bool,
) -> anyhow::Result<FullReport> {
    let timer = timing::RunTimer::start();
    let mut corpora = Vec::new();
    if report_kind != ReportKind::Performance {
        corpora.extend([run_engine_corpus(), run_test262_corpus()]);
        corpora.extend(test262_full::run_reports(test262)?);
        corpora.push(run_quickjs_corpus(quickjs));
    }
    let benchmarks = if matches!(report_kind, ReportKind::Full | ReportKind::Performance) {
        benchmarks::run()
    } else {
        benchmarks::BenchmarkReport::not_run()
    };
    let jetstream = if include_jetstream {
        jetstream::run()?
    } else {
        jetstream::JetStreamReport::not_run()
    };
    let elapsed = timer.elapsed();
    Ok(FullReport {
        metadata: report_metadata::RunMetadata::from_env(),
        corpora,
        benchmarks,
        jetstream,
        elapsed,
    })
}

fn run_engine_corpus() -> CorpusReport {
    let timer = timing::RunTimer::start();
    let cases = cases::engine_cases();
    let rows = cases.iter().map(run_engine_case).collect();
    CorpusReport::from_rows("Engine fixtures", rows, timer.elapsed())
}

fn run_test262_corpus() -> CorpusReport {
    let timer = timing::RunTimer::start();
    let cases = cases::test262_cases();
    let rows = cases.iter().map(run_engine_case).collect();
    CorpusReport::from_rows("Test262 active subset", rows, timer.elapsed())
}

fn run_quickjs_corpus(quickjs: Option<&Path>) -> CorpusReport {
    let timer = timing::RunTimer::start();
    let rows = cases::quickjs_differential_cases()
        .into_iter()
        .map(|case| run_differential_case(&case, quickjs))
        .collect();
    CorpusReport::from_rows("QuickJS differential", rows, timer.elapsed())
}

fn run_engine_case(case: &EngineCase) -> CaseRow {
    let result = timing::timed(|| execute_engine_case(case));
    match result.value {
        Ok(()) => CaseRow {
            case: case.id.to_owned(),
            status: STATUS_PASSED.to_owned(),
            source: case.path.to_owned(),
            detail: REASON_MATCHED.to_owned(),
            elapsed: result.elapsed,
        },
        Err(error) => CaseRow {
            case: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            detail: error.to_string(),
            elapsed: result.elapsed,
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
            let value = result.map_err(|error| {
                anyhow::anyhow!("case '{}' failed while evaluating source: {error}", case.id)
            })?;
            ensure_value(case.id, &value, expected)?;
            ensure_output(case.id, context.output(), &[])?;
        }
        Expectation::OutputAndValue { output, value } => {
            let actual = result.map_err(|error| {
                anyhow::anyhow!("case '{}' failed while evaluating source: {error}", case.id)
            })?;
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
            elapsed: Duration::ZERO,
        };
    };

    let result = timing::timed(|| execute_differential_case(case, quickjs));
    match result.value {
        Ok(()) => CaseRow {
            case: case.id.to_owned(),
            status: STATUS_PASSED.to_owned(),
            source: case.path.to_owned(),
            detail: "rs-quickjs stdout matched QuickJS stdout".to_owned(),
            elapsed: result.elapsed,
        },
        Err(error) => CaseRow {
            case: case.id.to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: case.path.to_owned(),
            detail: error.to_string(),
            elapsed: result.elapsed,
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
        .map_err(|error| anyhow::anyhow!("rs-quickjs evaluation failed: {error}"))?;
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

fn write_report(path: &Path, report: &ReportDocument) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create report directory '{}'", parent.display()))?;
    }

    let component = report.bounded_component()?;
    let exhaustive = report_schema_io::exhaustive_enabled().then_some(report);
    let yaml_paths = report_schema_io::write_yaml_artifacts(path, &component, exhaustive)?;
    println!(
        "structured YAML report summary: {}",
        yaml_paths.summary.display()
    );
    println!(
        "bounded YAML composition source: {}",
        yaml_paths.component.display()
    );
    if let Some(exhaustive_path) = &yaml_paths.exhaustive {
        println!(
            "exhaustive YAML report artifact: {}",
            exhaustive_path.display()
        );
    }
    let markdown_report = if report.configuration.report_mode == ReportMode::Jetstream {
        report
    } else {
        &component
    };
    let body = render_report(markdown_report);
    fs::write(path, body)
        .with_context(|| format!("failed to write test report '{}'", path.display()))?;
    let timing_path = timing_artifact_path(path);
    fs::write(&timing_path, render_timing_tsv(&component)).with_context(|| {
        format!(
            "failed to write timing artifact '{}'",
            timing_path.display()
        )
    })?;
    println!("bounded timing artifact: {}", timing_path.display());
    if exhaustive.is_some() {
        let exhaustive_timing_path = exhaustive_timing_artifact_path(path);
        fs::write(&exhaustive_timing_path, render_timing_tsv(report)).with_context(|| {
            format!(
                "failed to write exhaustive timing artifact '{}'",
                exhaustive_timing_path.display()
            )
        })?;
        println!(
            "exhaustive timing artifact: {}",
            exhaustive_timing_path.display()
        );
    }
    Ok(())
}

fn timing_artifact_path(report_path: &Path) -> PathBuf {
    let file_stem = report_path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("rsqjs-test-report");
    report_path.with_file_name(format!("{file_stem}-timings.tsv"))
}

fn exhaustive_timing_artifact_path(report_path: &Path) -> PathBuf {
    let file_stem = report_path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("rsqjs-test-report");
    report_path.with_file_name(format!("{file_stem}-exhaustive-timings.tsv"))
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
