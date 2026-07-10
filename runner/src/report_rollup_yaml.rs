use std::path::Path;

use anyhow::Context as _;

use crate::{
    report_schema::{ReportMode, ReportSummary, TEST262_FULL_SUITE},
    report_schema_io::read_summary,
};

use super::{ReportContext, ReportRecord, TestCounts, geomean, pr_task_title};

pub(super) fn parse(
    path: &Path,
    file_name: String,
    timestamp: String,
) -> anyhow::Result<ReportRecord> {
    let summary = read_summary(path)?;
    from_summary(file_name, timestamp, &summary)
}

pub(super) fn parse_jetstream(
    path: &Path,
    file_name: String,
    timestamp: String,
) -> anyhow::Result<ReportRecord> {
    let summary = read_summary(path)?;
    if summary.configuration.report_mode != ReportMode::Jetstream {
        anyhow::bail!(
            "structured JetStream report '{}' has mode {:?}",
            path.display(),
            summary.configuration.report_mode
        );
    }
    from_summary(file_name, timestamp, &summary)
}

fn from_summary(
    file_name: String,
    timestamp: String,
    summary: &ReportSummary,
) -> anyhow::Result<ReportRecord> {
    let latency_values = ratio_values(
        summary
            .benchmarks
            .rows
            .iter()
            .map(|row| row.latency_ratio_centi_units),
    )?;
    let memory_values = ratio_values(
        summary
            .benchmarks
            .rows
            .iter()
            .map(|row| row.memory_ratio_centi_units),
    )?;
    let jetstream_values = ratio_values(
        summary
            .jetstream
            .rows
            .iter()
            .map(|row| row.latency_ratio_centi_units),
    )?;
    Ok(ReportRecord {
        file_name,
        timestamp,
        benchmark_count: u64_to_usize(summary.benchmarks.counts.measured)?,
        latency_geomean: geomean(&latency_values),
        memory_geomean: geomean(&memory_values),
        jetstream_count: u64_to_usize(summary.jetstream.counts.measured)?,
        jetstream_latency_geomean: geomean(&jetstream_values),
        latency_over: u64_to_usize(summary.benchmarks.counts.over_latency_budget)?,
        memory_over: u64_to_usize(summary.benchmarks.counts.over_memory_budget)?,
        jetstream_latency_over: u64_to_usize(summary.jetstream.counts.over_latency_budget)?,
        benchmark_report: matches!(
            summary.configuration.report_mode,
            ReportMode::Full | ReportMode::Performance
        ),
        jetstream_report: summary.configuration.report_mode == ReportMode::Jetstream
            || summary.jetstream.counts.total > 0,
        full_test262: summary
            .suite(TEST262_FULL_SUITE)
            .map(|suite| test_counts(suite.counts))
            .transpose()?,
        context: report_context(summary),
    })
}

fn ratio_values(values: impl Iterator<Item = Option<u64>>) -> anyhow::Result<Vec<f64>> {
    let mut ratios = Vec::new();
    for value in values.flatten() {
        let value = u32::try_from(value).context("benchmark ratio exceeds rollup range")?;
        ratios.push(f64::from(value) / 100.0);
    }
    Ok(ratios)
}

fn test_counts(counts: crate::report_schema::CaseCounts) -> anyhow::Result<TestCounts> {
    Ok(TestCounts {
        total: u32::try_from(counts.total).context("Test262 total exceeds rollup range")?,
        passed: u32::try_from(counts.passed).context("Test262 passed exceeds rollup range")?,
        failed: u32::try_from(counts.failed).context("Test262 failed exceeds rollup range")?,
    })
}

fn report_context(summary: &ReportSummary) -> ReportContext {
    let mut task = summary.metadata.task.clone();
    if !summary.metadata.pull_request.is_empty() {
        task = pr_task_title(&summary.metadata.pull_request, &task);
    }
    ReportContext {
        task,
        purpose: String::new(),
        commit: summary.metadata.commit.chars().take(7).collect(),
    }
}

fn u64_to_usize(value: u64) -> anyhow::Result<usize> {
    usize::try_from(value).context("report count exceeds platform range")
}

#[cfg(test)]
mod tests {
    use super::from_summary;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn builds_rollup_record_from_structured_summary() -> TestResult {
        let summary = crate::report_schema_tests::sample_document()?.summary();
        let record = from_summary(
            "rsqjs-test-report-20260709T000000Z.md".to_owned(),
            "20260709T000000Z".to_owned(),
            &summary,
        )?;
        ensure_usize(record.benchmark_count, 1)?;
        ensure_usize(record.latency_over, 1)?;
        let Some(latency) = record.latency_geomean else {
            return Err("expected latency geometric mean".into());
        };
        ensure_close(latency, 1.25)
    }

    fn ensure_usize(actual: usize, expected: usize) -> TestResult {
        if actual == expected {
            return Ok(());
        }
        Err(format!("expected {expected}, got {actual}").into())
    }

    fn ensure_close(actual: f64, expected: f64) -> TestResult {
        if (actual - expected).abs() < f64::EPSILON {
            return Ok(());
        }
        Err(format!("expected {expected}, got {actual}").into())
    }
}
