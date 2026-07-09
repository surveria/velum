use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, bail};

#[path = "report_rollup_chart.rs"]
mod report_rollup_chart;
#[path = "report_rollup_jetstream.rs"]
mod report_rollup_jetstream;
#[path = "report_rollup_yaml.rs"]
mod report_rollup_yaml;

use report_rollup_chart::write_chart;

const REPORT_PREFIX: &str = "rsqjs-test-report-";
const REPORT_SUFFIX: &str = ".md";
const ROLLUP_FILE: &str = "benchmark-rollup.md";
const SUMMARY_CHART_FILE: &str = "benchmark-summary.jpg";
const PLAN_PATH: &str = "docs/project-plan.md";
const TEST262_FULL_SECTION: &str = "Test262 full corpus";
const METADATA_TESTED_COMMIT: &str = "Tested commit";
const METADATA_PULL_REQUEST: &str = "Pull request";
const METADATA_TASK: &str = "Task";
const BUDGET_LABEL: &str = "1.00x";
const BUDGET_RATIO: f64 = 1.00;

#[derive(Debug)]
pub struct RollupOutputs {
    pub markdown: PathBuf,
    pub summary_chart: PathBuf,
}

#[derive(Debug)]
struct RollupReport {
    records: Vec<ReportRecord>,
    outputs: RollupOutputs,
}

#[derive(Debug, Clone)]
struct ReportRecord {
    file_name: String,
    timestamp: String,
    benchmark_count: usize,
    latency_geomean: Option<f64>,
    memory_geomean: Option<f64>,
    jetstream_count: usize,
    jetstream_latency_geomean: Option<f64>,
    latency_over: usize,
    memory_over: usize,
    jetstream_latency_over: usize,
    benchmark_report: bool,
    jetstream_report: bool,
    full_test262: Option<TestCounts>,
    context: ReportContext,
}

#[derive(Debug, Clone, Copy)]
struct TestCounts {
    total: u32,
    passed: u32,
    failed: u32,
}

impl TestCounts {
    fn pass_rate(self) -> Option<f64> {
        if self.total == 0 {
            return None;
        }
        Some(f64::from(self.passed) * 100.0 / f64::from(self.total))
    }
}

#[derive(Debug, Clone, Default)]
struct ReportContext {
    task: String,
    purpose: String,
    commit: String,
}

pub fn generate_from_report_path(report_path: &Path) -> anyhow::Result<RollupOutputs> {
    let report_dir = report_path
        .parent()
        .context("test report path must have a parent directory")?;
    generate_from_report_dir(report_dir)
}

pub fn generate_from_report_dir(report_dir: &Path) -> anyhow::Result<RollupOutputs> {
    let rollup = build_rollup(report_dir)?;
    write_rollup(&rollup)?;
    Ok(rollup.outputs)
}

fn build_rollup(report_dir: &Path) -> anyhow::Result<RollupReport> {
    let report_dir = normalize_path(report_dir)?;
    let reports_root = report_dir
        .parent()
        .context("report directory must have a parent reports directory")?
        .to_path_buf();
    fs::create_dir_all(&reports_root).with_context(|| {
        format!(
            "failed to create rollup report directory '{}'",
            reports_root.display()
        )
    })?;

    let mut records = parse_records(&report_dir)?;
    attach_contexts(&mut records, &report_dir);
    if records.is_empty() {
        bail!(
            "no test or standalone JetStream reports found under '{}'",
            reports_root.display()
        );
    }

    Ok(RollupReport {
        records,
        outputs: RollupOutputs {
            markdown: reports_root.join(ROLLUP_FILE),
            summary_chart: reports_root.join(SUMMARY_CHART_FILE),
        },
    })
}

fn normalize_path(path: &Path) -> anyhow::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()
        .context("failed to read current directory")?
        .join(path))
}

fn parse_records(report_dir: &Path) -> anyhow::Result<Vec<ReportRecord>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(report_dir)
        .with_context(|| format!("failed to read report directory '{}'", report_dir.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "failed to read a report directory entry from '{}'",
                report_dir.display()
            )
        })?;
        let path = entry.path();
        if report_timestamp(&path).is_some() {
            paths.push(path);
        }
    }
    paths.sort_by_key(|path| report_timestamp(path).unwrap_or_default());

    let mut records = Vec::new();
    for path in paths {
        records.push(parse_report(&path)?);
    }
    let reports_root = report_dir
        .parent()
        .context("test report directory must have a reports parent")?;
    for (path, timestamp) in report_rollup_jetstream::structured_reports(reports_root)? {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("JetStream YAML file name must be valid UTF-8")?
            .to_owned();
        records.push(report_rollup_yaml::parse_jetstream(
            &path, file_name, timestamp,
        )?);
    }
    records.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then_with(|| left.file_name.cmp(&right.file_name))
    });
    Ok(records)
}

fn parse_report(path: &Path) -> anyhow::Result<ReportRecord> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("report file name must be valid UTF-8")?
        .to_owned();
    let timestamp = report_timestamp(path).context("report file name must include a timestamp")?;
    let summary_path = path.with_extension("yaml");
    if summary_path.is_file() {
        return report_rollup_yaml::parse(&summary_path, file_name, timestamp);
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read test report '{}'", path.display()))?;
    let parsed_benchmarks = parse_benchmark_metrics(&text);
    let parsed_jetstream = report_rollup_jetstream::parse_for_report(path, &text);
    Ok(ReportRecord {
        file_name,
        timestamp,
        benchmark_count: parsed_benchmarks.benchmark_count,
        latency_geomean: geomean(&parsed_benchmarks.latency_values),
        memory_geomean: geomean(&parsed_benchmarks.memory_values),
        jetstream_count: parsed_jetstream.benchmark_count,
        jetstream_latency_geomean: parsed_jetstream.latency_geomean,
        latency_over: parsed_benchmarks.latency_over,
        memory_over: parsed_benchmarks.memory_over,
        jetstream_latency_over: parsed_jetstream.latency_over,
        benchmark_report: true,
        jetstream_report: parsed_jetstream.benchmark_count > 0,
        full_test262: parse_rollup_test262_counts(&text),
        context: parse_report_metadata_context(&text),
    })
}

fn parse_report_metadata_context(text: &str) -> ReportContext {
    let mut context = ReportContext::default();
    let mut pull_request = String::new();
    for line in text.lines() {
        if let Some(commit) = metadata_value(line, METADATA_TESTED_COMMIT) {
            context.commit = commit.chars().take(7).collect();
            continue;
        }
        if let Some(task) = metadata_value(line, METADATA_TASK) {
            context.task = task;
            continue;
        }
        if let Some(value) = metadata_value(line, METADATA_PULL_REQUEST) {
            value.trim_start_matches('#').clone_into(&mut pull_request);
        }
    }
    if !pull_request.is_empty() {
        context.task = pr_task_title(&pull_request, &context.task);
    }
    context
}

fn metadata_value(line: &str, label: &str) -> Option<String> {
    let prefix = format!("- {label}: ");
    let value = line.strip_prefix(&prefix)?.trim();
    Some(value.trim_matches('`').to_owned())
}

fn pr_task_title(pull_request: &str, task: &str) -> String {
    if task.is_empty() {
        return format!("PR #{pull_request}");
    }
    format!("PR #{pull_request}: {task}")
}

fn report_timestamp(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let timestamp = file_name
        .strip_prefix(REPORT_PREFIX)?
        .strip_suffix(REPORT_SUFFIX)?;
    Some(timestamp.to_owned())
}

#[derive(Debug, Default)]
struct ParsedBenchmarks {
    benchmark_count: usize,
    latency_values: Vec<f64>,
    memory_values: Vec<f64>,
    latency_over: usize,
    memory_over: usize,
}

fn parse_benchmark_metrics(text: &str) -> ParsedBenchmarks {
    let mut parsed = ParsedBenchmarks::default();
    let mut in_benchmarks = false;
    let mut latency_index = None;
    let mut memory_index = None;
    let mut summary_latency_over = None;
    let mut summary_memory_over = None;
    let latency_summary_label = format!("Over latency budget ({BUDGET_LABEL})");
    let memory_summary_label = format!("Over memory budget ({BUDGET_LABEL})");

    for line in text.lines() {
        if line == "## Benchmarks" {
            in_benchmarks = true;
            continue;
        }
        if in_benchmarks && line.starts_with("## ") {
            break;
        }
        if !in_benchmarks {
            continue;
        }
        if let Some(count) = parse_summary_count(line, "Measured") {
            parsed.benchmark_count = count;
        }
        if let Some(count) = parse_summary_count(line, &latency_summary_label) {
            summary_latency_over = Some(count);
        }
        if let Some(count) = parse_summary_count(line, &memory_summary_label) {
            summary_memory_over = Some(count);
        }
        if !line.starts_with('|') {
            continue;
        }

        let cells = split_table_row(line);
        if cells.iter().any(|cell| cell == "benchmark") {
            latency_index = cells.iter().position(|cell| cell == "latency_ratio");
            memory_index = cells.iter().position(|cell| cell == "memory_ratio");
            continue;
        }
        record_benchmark_row(&mut parsed, &cells, latency_index, memory_index);
    }

    if parsed.benchmark_count == 0 {
        parsed.benchmark_count = parsed.latency_values.len();
    }
    parsed.latency_over =
        summary_latency_over.unwrap_or_else(|| count_over_budget(&parsed.latency_values));
    parsed.memory_over =
        summary_memory_over.unwrap_or_else(|| count_over_budget(&parsed.memory_values));
    parsed
}

fn record_benchmark_row(
    parsed: &mut ParsedBenchmarks,
    cells: &[String],
    latency_index: Option<usize>,
    memory_index: Option<usize>,
) {
    if let Some(value) = latency_index
        .and_then(|index| cells.get(index))
        .and_then(|cell| parse_ratio(cell))
    {
        parsed.latency_values.push(value);
    }
    if let Some(value) = memory_index
        .and_then(|index| cells.get(index))
        .and_then(|cell| parse_ratio(cell))
    {
        parsed.memory_values.push(value);
    }
}

fn count_over_budget(values: &[f64]) -> usize {
    values.iter().filter(|value| **value > BUDGET_RATIO).count()
}

fn split_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .map(str::to_owned)
        .collect()
}

fn parse_summary_count(line: &str, label: &str) -> Option<usize> {
    let suffix = line.trim().strip_prefix("- ")?;
    let value = suffix.strip_prefix(label)?.strip_prefix(": ")?;
    value.parse().ok()
}

fn parse_ratio(text: &str) -> Option<f64> {
    let ratio = text.trim().strip_suffix('x')?;
    ratio.parse().ok()
}

fn geomean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut log_sum = 0.0;
    let mut count = 0usize;
    for value in values {
        if *value <= 0.0 {
            continue;
        }
        log_sum += value.ln();
        count = count.saturating_add(1);
    }
    if count == 0 {
        return None;
    }
    Some((log_sum / usize_to_f64(count)?).exp())
}

fn parse_corpus_counts(text: &str, section: &str) -> Option<TestCounts> {
    let mut in_section = false;
    let mut total = None;
    let mut passed = None;
    let mut failed = None;
    let heading = format!("## {section}");
    for line in text.lines() {
        if line == heading {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if !in_section {
            continue;
        }
        if let Some(value) = parse_summary_u32(line, "Total") {
            total = Some(value);
        }
        if let Some(value) = parse_summary_u32(line, "Passed") {
            passed = Some(value);
        }
        if let Some(value) = parse_summary_u32(line, "Failed") {
            failed = Some(value);
        }
    }
    Some(TestCounts {
        total: total?,
        passed: passed?,
        failed: failed?,
    })
}

fn parse_rollup_test262_counts(text: &str) -> Option<TestCounts> {
    parse_corpus_counts(text, TEST262_FULL_SECTION)
}

fn parse_summary_u32(line: &str, label: &str) -> Option<u32> {
    let suffix = line.trim().strip_prefix("- ")?;
    let value = suffix.strip_prefix(label)?.strip_prefix(": ")?;
    value.parse().ok()
}

fn usize_to_f64(value: usize) -> Option<f64> {
    let value = u32::try_from(value).ok()?;
    Some(f64::from(value))
}

fn attach_contexts(records: &mut [ReportRecord], report_dir: &Path) {
    let mut contexts = plan_contexts();
    for (file_name, context) in git_contexts(report_dir) {
        contexts
            .entry(file_name)
            .and_modify(|existing| {
                existing.commit.clone_from(&context.commit);
                if existing.task.is_empty() {
                    existing.task.clone_from(&context.task);
                }
            })
            .or_insert(context);
    }

    for record in records {
        if let Some(context) = contexts.get(&record.file_name) {
            merge_context(&mut record.context, context);
        }
    }
}

fn merge_context(target: &mut ReportContext, source: &ReportContext) {
    if target.commit.is_empty() {
        target.commit.clone_from(&source.commit);
    }
    if target.task.is_empty() {
        target.task.clone_from(&source.task);
    }
    if target.purpose.is_empty() {
        target.purpose.clone_from(&source.purpose);
    }
}

fn plan_contexts() -> BTreeMap<String, ReportContext> {
    let path = Path::new(PLAN_PATH);
    let Ok(text) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let mut contexts = BTreeMap::new();
    for line in text.lines() {
        if !line.starts_with("| [") {
            continue;
        }
        let cells = split_table_row(line);
        let Some(task) = cells.get(2) else {
            continue;
        };
        let purpose = cells.get(4).cloned().unwrap_or_default();
        let notes = cells.get(5).cloned().unwrap_or_default();
        for file_name in report_names_in_text(&notes) {
            contexts.insert(
                file_name,
                ReportContext {
                    task: task.clone(),
                    purpose: purpose.clone(),
                    commit: String::new(),
                },
            );
        }
    }
    contexts
}

fn report_names_in_text(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter_map(|word| {
            let clean = word.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '-' && character != '.'
            });
            if clean.starts_with(REPORT_PREFIX) && clean.ends_with(REPORT_SUFFIX) {
                return Some(clean.to_owned());
            }
            None
        })
        .collect()
}

fn git_contexts(report_dir: &Path) -> BTreeMap<String, ReportContext> {
    let pathspec = git_pathspec(report_dir);
    let Ok(output) = Command::new("git")
        .arg("log")
        .arg("--reverse")
        .arg("--diff-filter=A")
        .arg("--format=commit%x09%H%x09%s")
        .arg("--name-only")
        .arg("--")
        .arg(pathspec)
        .output()
    else {
        return BTreeMap::new();
    };
    if !output.status.success() {
        return BTreeMap::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_git_log(&text)
}

fn git_pathspec(report_dir: &Path) -> PathBuf {
    let Ok(current_dir) = std::env::current_dir() else {
        return report_dir.to_path_buf();
    };
    let Ok(relative) = report_dir.strip_prefix(current_dir) else {
        return report_dir.to_path_buf();
    };
    relative.to_path_buf()
}

fn parse_git_log(text: &str) -> BTreeMap<String, ReportContext> {
    let mut contexts = BTreeMap::new();
    let mut current = ReportContext::default();
    for line in text.lines() {
        if let Some(commit_line) = line.strip_prefix("commit\t") {
            current = parse_commit_line(commit_line);
            continue;
        }
        let path = Path::new(line);
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.starts_with(REPORT_PREFIX) && file_name.ends_with(REPORT_SUFFIX) {
            contexts.insert(file_name.to_owned(), current.clone());
        }
    }
    contexts
}

fn parse_commit_line(line: &str) -> ReportContext {
    let mut fields = line.splitn(2, '\t');
    let commit = fields.next().unwrap_or_default();
    let task = fields.next().unwrap_or_default();
    ReportContext {
        task: task.to_owned(),
        purpose: String::new(),
        commit: commit.chars().take(7).collect(),
    }
}

fn write_rollup(rollup: &RollupReport) -> anyhow::Result<()> {
    let markdown = render_markdown(&rollup.records);
    fs::write(&rollup.outputs.markdown, markdown).with_context(|| {
        format!(
            "failed to write benchmark rollup '{}'",
            rollup.outputs.markdown.display()
        )
    })?;
    write_chart(&rollup.records, &rollup.outputs.summary_chart)
}

fn render_markdown(records: &[ReportRecord]) -> String {
    let mut lines = vec![
        "# rs-quickjs Benchmark Rollup".to_owned(),
        String::new(),
        "Generated by `rsqjs-test-runner` from tracked test reports.".to_owned(),
        String::new(),
        "Metric definition:".to_owned(),
        String::new(),
        "- Performance is the geometric mean of benchmark `latency_ratio` values versus QuickJS."
            .to_owned(),
        "- Memory is the geometric mean of benchmark `memory_ratio` values versus QuickJS."
            .to_owned(),
        "- JetStream is the geometric mean of shell benchmark `latency_ratio` values versus QuickJS."
            .to_owned(),
        "- Full Test262 shows passed and failed case counts from the full Test262 corpus."
            .to_owned(),
        "- `1.00x` means QuickJS parity; lower performance, memory, and JetStream ratios are better."
            .to_owned(),
        "- Parentheses show budget exceptions over measured rows for each benchmark family."
            .to_owned(),
        String::new(),
        "Artifacts:".to_owned(),
        String::new(),
        format!("- `{SUMMARY_CHART_FILE}`"),
        String::new(),
    ];
    append_latest_section(&mut lines, records);
    lines.extend([
        "| PR / task | Performance | Memory | JetStream | Full Test262 |".to_owned(),
        "| --- | ---: | ---: | ---: | ---: |".to_owned(),
    ]);
    for record in records {
        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            escape_cell(&record_label(record)),
            metric_text(
                record.latency_geomean,
                record.latency_over,
                record.benchmark_count
            ),
            metric_text(
                record.memory_geomean,
                record.memory_over,
                record.benchmark_count
            ),
            jetstream_metric_text(
                record.jetstream_latency_geomean,
                record.jetstream_latency_over,
                record.jetstream_count
            ),
            test_counts_text(record.full_test262),
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn append_latest_section(lines: &mut Vec<String>, records: &[ReportRecord]) {
    let Some(latest) = records.last() else {
        return;
    };
    let latest_benchmark = records.iter().rev().find(|record| record.benchmark_report);
    let latest_jetstream = records.iter().rev().find(|record| record.jetstream_report);
    let latest_test262 = records
        .iter()
        .rev()
        .find(|record| record.full_test262.is_some());
    lines.extend([
        "Latest report:".to_owned(),
        String::new(),
        format!("- `{}`", latest.file_name),
        format!("- Task: {}", record_title(latest)),
        latest_metric_line("Performance", latest_benchmark, |record| {
            metric_text(
                record.latency_geomean,
                record.latency_over,
                record.benchmark_count,
            )
        }),
        latest_metric_line("Memory", latest_benchmark, |record| {
            metric_text(
                record.memory_geomean,
                record.memory_over,
                record.benchmark_count,
            )
        }),
        latest_metric_line("JetStream", latest_jetstream, |record| {
            jetstream_metric_text(
                record.jetstream_latency_geomean,
                record.jetstream_latency_over,
                record.jetstream_count,
            )
        }),
        latest_metric_line("Full Test262", latest_test262, |record| {
            test_counts_text(record.full_test262)
        }),
        String::new(),
    ]);
}

fn latest_metric_line(
    label: &str,
    record: Option<&ReportRecord>,
    value: impl FnOnce(&ReportRecord) -> String,
) -> String {
    let Some(record) = record else {
        return format!("- {label}: -");
    };
    format!("- {label}: {} (from `{}`)", value(record), record.file_name)
}

fn record_label(record: &ReportRecord) -> String {
    let title = record_title(record);
    if record.context.commit.is_empty() {
        return format!("`{}` {}", record.timestamp, title);
    }
    format!(
        "`{}` `{}` {}",
        record.timestamp, record.context.commit, title
    )
}

fn record_title(record: &ReportRecord) -> String {
    if record.context.task.is_empty() {
        return record.file_name.clone();
    }
    if record.context.purpose.is_empty() {
        return record.context.task.clone();
    }
    format!("{}: {}", record.context.task, record.context.purpose)
}

fn metric_text(value: Option<f64>, over: usize, total: usize) -> String {
    let ratio = value.map_or_else(|| "-".to_owned(), |value| format!("{value:.2}x"));
    format!("{ratio} ({over}/{total} >{BUDGET_LABEL})")
}

fn jetstream_metric_text(value: Option<f64>, over: usize, total: usize) -> String {
    let ratio = value.map_or_else(|| "-".to_owned(), |value| format!("{value:.2}x"));
    format!("{ratio} ({over}/{total} >{BUDGET_LABEL})")
}

fn percent_text(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| format!("{value:.2}%"))
}

fn test_counts_text(value: Option<TestCounts>) -> String {
    let Some(value) = value else {
        return "-".to_owned();
    };
    format!(
        "{} passed / {} failed ({})",
        value.passed,
        value.failed,
        percent_text(value.pass_rate())
    )
}

fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|")
        .replace('\n', " ")
        .trim()
        .to_owned()
}

#[cfg(test)]
#[path = "report_rollup_tests.rs"]
mod tests;
