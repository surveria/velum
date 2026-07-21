use std::{
    fs::{self, OpenOptions},
    io::{BufRead as _, BufReader, Write as _},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;
use tabled::{Table, Tabled};

use crate::compare::{CaseClassification, CaseRecord, OutcomeStatus};

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
    matches: u64,
    mismatches: u64,
    slow: u64,
    v8_timeouts: u64,
    v8_crashes: u64,
    velum_js_errors: u64,
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
    let summary_path = session_dir.join("summary.txt");
    let table = Table::new(rows(&summary, elapsed, outcome, latest_findings.len())).to_string();
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
    summary: &Summary,
    elapsed: Duration,
    outcome: &str,
    finding_files: usize,
) -> Vec<SummaryRow> {
    vec![
        row("Fuzzilli outcome", outcome),
        row("Elapsed", &humantime::format_duration(elapsed).to_string()),
        row("Compared scripts", &summary.total.to_string()),
        row("Equivalent results", &summary.matches.to_string()),
        row("Mismatches", &summary.mismatches.to_string()),
        row("Slow equivalent cases", &summary.slow.to_string()),
        row("V8 timeouts", &summary.v8_timeouts.to_string()),
        row("V8 crashes", &summary.v8_crashes.to_string()),
        row("Velum JS errors", &summary.velum_js_errors.to_string()),
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
        match record.classification {
            CaseClassification::Match => self.matches = self.matches.saturating_add(1),
            CaseClassification::Mismatch => self.mismatches = self.mismatches.saturating_add(1),
            CaseClassification::Slow => self.slow = self.slow.saturating_add(1),
            CaseClassification::V8Timeout => self.v8_timeouts = self.v8_timeouts.saturating_add(1),
            CaseClassification::V8Crash => self.v8_crashes = self.v8_crashes.saturating_add(1),
        }
        if record.velum.status == OutcomeStatus::JsError {
            self.velum_js_errors = self.velum_js_errors.saturating_add(1);
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
        "ratio\tcase_id\tclassification\tvelum_ns\tv8_ns\tsaved_script"
    )
    .with_context(|| format!("failed to write '{}'", path.display()))?;
    for (ratio, record) in sorted.into_iter().take(100) {
        writeln!(
            file,
            "{}\t{}\t{:?}\t{}\t{}\t{}",
            format_ratio(ratio),
            record.case_id,
            record.classification,
            record.velum.elapsed_nanos,
            record.v8.elapsed_nanos,
            record.saved_script.as_deref().unwrap_or("")
        )
        .with_context(|| format!("failed to write '{}'", path.display()))?;
    }
    Ok(())
}

fn format_ratio(value: f64) -> String {
    format!("{value:.3}x")
}
