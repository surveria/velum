use std::{
    collections::BTreeMap,
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
    correctness::CorrectnessEvaluation,
    reference_gaps::{OracleDecision, OracleUnavailableReason, ReferenceGapReason},
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
    correctness_equivalent: u64,
    correctness_mismatches: u64,
    correctness_unverified: u64,
    legacy_untyped: u64,
    engine262_oracle_selected: u64,
    v8_fallback_selected: u64,
    oracle_unavailable: u64,
    performance_slow: u64,
    velum_timeouts: u64,
    velum_crashes: u64,
    velum_resource_limits: u64,
    engine262_timeouts: u64,
    engine262_crashes: u64,
    engine262_unsupported: u64,
    v8_timeouts: u64,
    v8_crashes: u64,
    velum_js_errors: u64,
    engine262_js_errors: u64,
    v8_js_errors: u64,
    ratio_sum: f64,
    ratio_count: u64,
    max_ratio: Option<f64>,
    max_ratio_case: Option<String>,
    reference_gap_counts: BTreeMap<ReferenceGapReason, u64>,
    oracle_unavailable_counts: BTreeMap<OracleUnavailableReason, u64>,
}

#[derive(Tabled)]
struct SummaryRow {
    #[tabled(rename = "Metric")]
    metric: &'static str,
    #[tabled(rename = "Value")]
    value: String,
}

#[derive(Tabled)]
struct DetailRow {
    #[tabled(rename = "Typed reason")]
    reason: &'static str,
    #[tabled(rename = "Cases")]
    cases: u64,
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
    let table = render_tables(
        rows(
            session_dir,
            &summary,
            elapsed,
            outcome,
            latest_findings.len(),
            pending_count,
        ),
        &summary,
    );
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
            "Correctness verified",
            &summary.correctness_verified().to_string(),
        ),
        row(
            "Correctness equivalent",
            &summary.correctness_equivalent.to_string(),
        ),
        row(
            "Correctness mismatches",
            &summary.correctness_mismatches.to_string(),
        ),
        row(
            "Correctness unverified",
            &summary.correctness_unverified.to_string(),
        ),
        row(
            "Engine262 oracle selected",
            &summary.engine262_oracle_selected.to_string(),
        ),
        row(
            "V8 fallback selected",
            &summary.v8_fallback_selected.to_string(),
        ),
        row(
            "No reliable oracle",
            &summary.oracle_unavailable.to_string(),
        ),
        row(
            "Legacy records without typed verdict",
            &summary.legacy_untyped.to_string(),
        ),
        row(
            "Performance slow cases",
            &summary.performance_slow.to_string(),
        ),
        row("Velum timeouts", &summary.velum_timeouts.to_string()),
        row("Velum crashes", &summary.velum_crashes.to_string()),
        row(
            "Velum resource limits",
            &summary.velum_resource_limits.to_string(),
        ),
        row(
            "Engine262 timeouts",
            &summary.engine262_timeouts.to_string(),
        ),
        row("Engine262 crashes", &summary.engine262_crashes.to_string()),
        row(
            "Engine262 unsupported",
            &summary.engine262_unsupported.to_string(),
        ),
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

fn render_tables(rows: Vec<SummaryRow>, summary: &Summary) -> String {
    let mut output = Table::new(rows).to_string();
    if !summary.reference_gap_counts.is_empty() {
        let details = summary
            .reference_gap_counts
            .iter()
            .map(|(reason, cases)| DetailRow {
                reason: reason.as_str(),
                cases: *cases,
            })
            .collect::<Vec<_>>();
        output.push_str("\n\nEngine262 reference gap reasons\n");
        output.push_str(&Table::new(details).to_string());
    }
    if !summary.oracle_unavailable_counts.is_empty() {
        let details = summary
            .oracle_unavailable_counts
            .iter()
            .map(|(reason, cases)| DetailRow {
                reason: reason.as_str(),
                cases: *cases,
            })
            .collect::<Vec<_>>();
        output.push_str("\n\nNo-reliable-oracle reasons\n");
        output.push_str(&Table::new(details).to_string());
    }
    output
}

impl Summary {
    fn add(&mut self, record: &CaseRecord) {
        self.total = self.total.saturating_add(1);
        let findings = normalized_findings(record);
        let legacy_untyped = matches!(
            record.correctness_evaluation,
            CorrectnessEvaluation::LegacyUnspecified
        );
        match &record.correctness_evaluation {
            CorrectnessEvaluation::Equivalent { .. } => {
                self.correctness_equivalent = self.correctness_equivalent.saturating_add(1);
            }
            CorrectnessEvaluation::Mismatch { .. } => {
                self.correctness_mismatches = self.correctness_mismatches.saturating_add(1);
            }
            CorrectnessEvaluation::Unverified { .. } => {
                self.correctness_unverified = self.correctness_unverified.saturating_add(1);
            }
            CorrectnessEvaluation::LegacyUnspecified => {
                self.legacy_untyped = self.legacy_untyped.saturating_add(1);
            }
        }
        match &record.reference_analysis.oracle {
            OracleDecision::Engine262 => {
                self.engine262_oracle_selected = self.engine262_oracle_selected.saturating_add(1);
            }
            OracleDecision::V8Fallback => {
                self.v8_fallback_selected = self.v8_fallback_selected.saturating_add(1);
            }
            OracleDecision::Unavailable { reasons } => {
                self.oracle_unavailable = self.oracle_unavailable.saturating_add(1);
                for reason in reasons {
                    increment_count(&mut self.oracle_unavailable_counts, *reason);
                }
            }
            OracleDecision::LegacyUnspecified => {}
        }
        for reason in &record.reference_analysis.engine262_gaps {
            increment_count(&mut self.reference_gap_counts, *reason);
        }
        for finding in &findings {
            match finding {
                CaseFinding::CorrectnessMismatch => {
                    if legacy_untyped {
                        self.correctness_mismatches = self.correctness_mismatches.saturating_add(1);
                    }
                }
                CaseFinding::CorrectnessUnverified => {
                    if legacy_untyped {
                        self.correctness_unverified = self.correctness_unverified.saturating_add(1);
                    }
                }
                CaseFinding::PerformanceSlow => {
                    self.performance_slow = self.performance_slow.saturating_add(1);
                }
                CaseFinding::VelumTimeout => {
                    self.velum_timeouts = self.velum_timeouts.saturating_add(1);
                }
                CaseFinding::VelumCrash => {
                    self.velum_crashes = self.velum_crashes.saturating_add(1);
                }
                CaseFinding::VelumResourceLimit => {
                    self.velum_resource_limits = self.velum_resource_limits.saturating_add(1);
                }
                CaseFinding::Engine262Timeout => {
                    self.engine262_timeouts = self.engine262_timeouts.saturating_add(1);
                }
                CaseFinding::Engine262Crash => {
                    self.engine262_crashes = self.engine262_crashes.saturating_add(1);
                }
                CaseFinding::Engine262Unsupported => {
                    self.engine262_unsupported = self.engine262_unsupported.saturating_add(1);
                }
                CaseFinding::V8Timeout => {
                    self.v8_timeouts = self.v8_timeouts.saturating_add(1);
                }
                CaseFinding::V8Crash => {
                    self.v8_crashes = self.v8_crashes.saturating_add(1);
                }
            }
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

    const fn correctness_verified(&self) -> u64 {
        self.correctness_equivalent
            .saturating_add(self.correctness_mismatches)
    }
}

fn increment_count<T: Copy + Ord>(counts: &mut BTreeMap<T, u64>, key: T) {
    let count = counts.entry(key).or_insert(0);
    *count = count.saturating_add(1);
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
        "ratio\tcase_id\tclassification\tfindings\tcorrectness\toracle\treference_gaps\tvelum_ns\tv8_ns\tsaved_scripts"
    )
    .with_context(|| format!("failed to write '{}'", path.display()))?;
    for (ratio, record) in sorted.into_iter().take(100) {
        writeln!(
            file,
            "{}\t{}\t{:?}\t{:?}\t{:?}\t{:?}\t{:?}\t{}\t{}\t{}",
            format_ratio(ratio),
            record.case_id,
            record.classification,
            normalized_findings(record),
            record.correctness_evaluation,
            record.reference_analysis.oracle,
            record.reference_analysis.engine262_gaps,
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
