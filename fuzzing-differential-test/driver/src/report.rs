use std::{
    fs::{self, OpenOptions},
    io::{BufRead as _, BufReader, Write as _},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;
use tabled::{Table, Tabled};

use crate::{
    artifacts::normalized_findings,
    compare::{CaseFinding, CaseRecord, OutcomeStatus},
};

const LATEST_FINDING_LIMIT: usize = 10;

#[derive(Debug)]
pub struct DifferentialReport {
    table: String,
    latest_findings: Vec<PathBuf>,
    summary_path: PathBuf,
}

impl DifferentialReport {
    #[must_use]
    pub fn render(&self) -> String {
        let mut output = format!("{}\nSummary: {}", self.table, self.summary_path.display());
        if self.latest_findings.is_empty() {
            output.push_str("\nNo finding scripts were saved.");
            return output;
        }
        output.push_str("\nLatest saved finding scripts (showing ");
        output.push_str(
            &self
                .latest_findings
                .len()
                .min(LATEST_FINDING_LIMIT)
                .to_string(),
        );
        output.push_str(" of ");
        output.push_str(&self.latest_findings.len().to_string());
        output.push_str("):");
        for path in self.latest_findings.iter().take(LATEST_FINDING_LIMIT) {
            output.push_str("\n- ");
            output.push_str(&path.display().to_string());
        }
        output
    }
}

#[derive(Default)]
struct Summary {
    total: u64,
    engine262_equivalent: u64,
    correctness_mismatches: u64,
    performance_slow: u64,
    velum_timeouts: u64,
    velum_crashes: u64,
    engine262_timeouts: u64,
    engine262_crashes: u64,
    v8_timeouts: u64,
    v8_crashes: u64,
    velum_js_errors: u64,
    engine262_js_errors: u64,
    v8_js_errors: u64,
    ratio_sum: f64,
    ratio_count: u64,
    max_ratio: Option<f64>,
    max_ratio_case: Option<String>,
}

#[derive(Tabled)]
struct SummaryRow {
    #[tabled(rename = "Metric")]
    metric: &'static str,
    #[tabled(rename = "Value")]
    value: String,
}

/// Builds and stores a differential fuzzing summary.
///
/// # Errors
///
/// Returns an error when case JSONL files or summary files cannot be read or
/// written.
pub fn build_report(
    session_dir: &Path,
    elapsed: Duration,
    outcome: &str,
) -> anyhow::Result<DifferentialReport> {
    let records = read_records(&session_dir.join("cases"))?;
    let summary = summarize(&records);
    let latest_findings = latest_javascript_files(&session_dir.join("findings"))?;
    let pending_count = javascript_file_count(&session_dir.join("pending"))?;
    let summary_path = session_dir.join("summary.txt");
    let table = Table::new(rows(
        session_dir,
        &summary,
        elapsed,
        outcome,
        latest_findings.len(),
        pending_count,
    ))
    .to_string();
    let report = DifferentialReport {
        table,
        latest_findings,
        summary_path,
    };
    fs::write(&report.summary_path, report.render()).with_context(|| {
        format!(
            "failed to write differential summary '{}'",
            report.summary_path.display()
        )
    })?;
    append_jsonl_listing(session_dir, &records)?;
    Ok(report)
}

fn rows(
    session_dir: &Path,
    summary: &Summary,
    elapsed: Duration,
    outcome: &str,
    finding_files: usize,
    pending_files: usize,
) -> Vec<SummaryRow> {
    vec![
        row("Run outcome", outcome),
        row("Artifact directory", &session_dir.display().to_string()),
        row("Elapsed", &humantime::format_duration(elapsed).to_string()),
        row("Compared scripts", &summary.total.to_string()),
        row(
            "Engine262-equivalent scripts",
            &summary.engine262_equivalent.to_string(),
        ),
        row(
            "Correctness mismatches",
            &summary.correctness_mismatches.to_string(),
        ),
        row(
            "Performance slow cases",
            &summary.performance_slow.to_string(),
        ),
        row("Velum timeouts", &summary.velum_timeouts.to_string()),
        row("Velum crashes", &summary.velum_crashes.to_string()),
        row(
            "Engine262 timeouts",
            &summary.engine262_timeouts.to_string(),
        ),
        row("Engine262 crashes", &summary.engine262_crashes.to_string()),
        row("V8 timeouts", &summary.v8_timeouts.to_string()),
        row("V8 crashes", &summary.v8_crashes.to_string()),
        row("Velum JS errors", &summary.velum_js_errors.to_string()),
        row(
            "Engine262 JS errors",
            &summary.engine262_js_errors.to_string(),
        ),
        row("V8 JS errors", &summary.v8_js_errors.to_string()),
        row(
            "Mean Velum/V8 ratio",
            &summary
                .mean_ratio()
                .map_or_else(|| "unavailable".to_owned(), format_ratio),
        ),
        row(
            "Max Velum/V8 ratio",
            &summary
                .max_ratio
                .map_or_else(|| "unavailable".to_owned(), format_ratio),
        ),
        row(
            "Max-ratio case",
            summary.max_ratio_case.as_deref().unwrap_or("unavailable"),
        ),
        row("Saved finding scripts", &finding_files.to_string()),
        row("Pending Velum abort candidates", &pending_files.to_string()),
    ]
}

fn row(metric: &'static str, value: &str) -> SummaryRow {
    SummaryRow {
        metric,
        value: value.to_owned(),
    }
}

impl Summary {
    fn add(&mut self, record: &CaseRecord) {
        self.total = self.total.saturating_add(1);
        let findings = normalized_findings(record);
        let mut has_correctness_problem = false;
        for finding in &findings {
            match finding {
                CaseFinding::CorrectnessMismatch => {
                    self.correctness_mismatches = self.correctness_mismatches.saturating_add(1);
                    has_correctness_problem = true;
                }
                CaseFinding::PerformanceSlow => {
                    self.performance_slow = self.performance_slow.saturating_add(1);
                }
                CaseFinding::VelumTimeout => {
                    self.velum_timeouts = self.velum_timeouts.saturating_add(1);
                    has_correctness_problem = true;
                }
                CaseFinding::VelumCrash => {
                    self.velum_crashes = self.velum_crashes.saturating_add(1);
                    has_correctness_problem = true;
                }
                CaseFinding::Engine262Timeout => {
                    self.engine262_timeouts = self.engine262_timeouts.saturating_add(1);
                    has_correctness_problem = true;
                }
                CaseFinding::Engine262Crash => {
                    self.engine262_crashes = self.engine262_crashes.saturating_add(1);
                    has_correctness_problem = true;
                }
                CaseFinding::V8Timeout => {
                    self.v8_timeouts = self.v8_timeouts.saturating_add(1);
                }
                CaseFinding::V8Crash => {
                    self.v8_crashes = self.v8_crashes.saturating_add(1);
                }
            }
        }
        if !has_correctness_problem {
            self.engine262_equivalent = self.engine262_equivalent.saturating_add(1);
        }
        if record.velum.status == OutcomeStatus::JsError {
            self.velum_js_errors = self.velum_js_errors.saturating_add(1);
        }
        if record.engine262.status == OutcomeStatus::JsError {
            self.engine262_js_errors = self.engine262_js_errors.saturating_add(1);
        }
        if record.v8.status == OutcomeStatus::JsError {
            self.v8_js_errors = self.v8_js_errors.saturating_add(1);
        }
        if let Some(ratio) = record.ratio_velum_to_v8
            && ratio.is_finite()
        {
            self.ratio_sum += ratio;
            self.ratio_count = self.ratio_count.saturating_add(1);
            if self.max_ratio.is_none_or(|value| ratio > value) {
                self.max_ratio = Some(ratio);
                self.max_ratio_case = Some(record.case_id.clone());
            }
        }
    }

    fn mean_ratio(&self) -> Option<f64> {
        if self.ratio_count == 0 {
            return None;
        }
        #[allow(clippy::cast_precision_loss)]
        Some(self.ratio_sum / self.ratio_count as f64)
    }
}

fn summarize(records: &[CaseRecord]) -> Summary {
    let mut summary = Summary::default();
    for record in records {
        summary.add(record);
    }
    summary
}

fn read_records(directory: &Path) -> anyhow::Result<Vec<CaseRecord>> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read '{}'", directory.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read '{}'", directory.display()))?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut records = Vec::new();
    for path in paths {
        let file = fs::File::open(&path)
            .with_context(|| format!("failed to open case log '{}'", path.display()))?;
        for line in BufReader::new(file).lines() {
            let line =
                line.with_context(|| format!("failed to read case log '{}'", path.display()))?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(
                serde_json::from_str(&line).with_context(|| {
                    format!("failed to parse case record in '{}'", path.display())
                })?,
            );
        }
    }
    Ok(records)
}

fn latest_javascript_files(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_javascript_files(directory, &mut files)?;
    files.sort_by(|left, right| right.cmp(left));
    Ok(files)
}

fn collect_javascript_files(directory: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("failed to read '{}'", directory.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read '{}'", directory.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_javascript_files(&path, files)?;
        } else if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some("js")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn javascript_file_count(directory: &Path) -> anyhow::Result<usize> {
    Ok(latest_javascript_files(directory)?.len())
}

fn append_jsonl_listing(session_dir: &Path, records: &[CaseRecord]) -> anyhow::Result<()> {
    let path = session_dir.join("slowest.tsv");
    let mut sorted = records
        .iter()
        .filter_map(|record| record.ratio_velum_to_v8.map(|ratio| (ratio, record)))
        .collect::<Vec<_>>();
    sorted.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .with_context(|| format!("failed to write '{}'", path.display()))?;
    writeln!(
        file,
        "ratio\tcase_id\tclassification\tfindings\tvelum_ns\tv8_ns\tsaved_scripts"
    )
    .with_context(|| format!("failed to write '{}'", path.display()))?;
    for (ratio, record) in sorted.into_iter().take(100) {
        writeln!(
            file,
            "{}\t{}\t{:?}\t{:?}\t{}\t{}\t{}",
            format_ratio(ratio),
            record.case_id,
            record.classification,
            normalized_findings(record),
            record.velum.elapsed_nanos,
            record.v8.elapsed_nanos,
            saved_scripts_text(record)
        )
        .with_context(|| format!("failed to write '{}'", path.display()))?;
    }
    Ok(())
}

fn saved_scripts_text(record: &CaseRecord) -> String {
    if !record.saved_scripts.is_empty() {
        return record.saved_scripts.join(",");
    }
    record.saved_script.clone().unwrap_or_default()
}

fn format_ratio(value: f64) -> String {
    format!("{value:.3}x")
}
