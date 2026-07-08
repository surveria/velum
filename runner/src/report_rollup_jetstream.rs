use std::{
    fs,
    path::{Path, PathBuf},
};

const JETSTREAM_REPORT_PREFIX: &str = "rsqjs-jetstream-report-";
const JETSTREAM_REPORT_SUFFIX: &str = ".md";
const JETSTREAM_REPORT_DIR: &str = "jetstream-runs";
const JETSTREAM_SECTION: &str = "JetStream Shell Benchmarks";
const TEST_REPORT_PREFIX: &str = "rsqjs-test-report-";
const TEST_REPORT_SUFFIX: &str = ".md";
const BUDGET_RATIO: f64 = 1.00;

#[derive(Debug, Clone, Copy, Default)]
pub struct JetStreamMetrics {
    pub benchmark_count: usize,
    pub score_geomean: Option<f64>,
    pub score_below: usize,
}

#[derive(Debug, Default)]
struct ParsedJetStream {
    benchmark_count: usize,
    score_values: Vec<f64>,
    score_below: usize,
}

pub fn parse_for_report(report_path: &Path, fallback_text: &str) -> JetStreamMetrics {
    let Some(timestamp) = report_timestamp(report_path) else {
        return parse_metrics(fallback_text);
    };
    let Some(path) = jetstream_report_path(report_path, &timestamp) else {
        return parse_metrics(fallback_text);
    };
    let Ok(text) = fs::read_to_string(path) else {
        return parse_metrics(fallback_text);
    };
    parse_metrics(&text)
}

fn jetstream_report_path(report_path: &Path, timestamp: &str) -> Option<PathBuf> {
    let reports_root = report_path.parent()?.parent()?;
    Some(reports_root.join(JETSTREAM_REPORT_DIR).join(format!(
        "{JETSTREAM_REPORT_PREFIX}{timestamp}{JETSTREAM_REPORT_SUFFIX}"
    )))
}

fn report_timestamp(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let timestamp = file_name
        .strip_prefix(TEST_REPORT_PREFIX)?
        .strip_suffix(TEST_REPORT_SUFFIX)?;
    Some(timestamp.to_owned())
}

fn parse_metrics(text: &str) -> JetStreamMetrics {
    let parsed = parse_jetstream_metrics(text);
    JetStreamMetrics {
        benchmark_count: parsed.benchmark_count,
        score_geomean: geomean(&parsed.score_values),
        score_below: parsed.score_below,
    }
}

fn parse_jetstream_metrics(text: &str) -> ParsedJetStream {
    let mut parsed = ParsedJetStream::default();
    let mut in_section = false;
    let mut score_index = None;
    let score_summary_label = format!("Below score budget ({})", super::BUDGET_LABEL);
    let mut summary_score_below = None;

    for line in text.lines() {
        if line == format!("## {JETSTREAM_SECTION}") {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if !in_section {
            continue;
        }
        if let Some(count) = parse_summary_count(line, "Measured") {
            parsed.benchmark_count = count;
        }
        if let Some(count) = parse_summary_count(line, &score_summary_label) {
            summary_score_below = Some(count);
        }
        if !line.starts_with('|') {
            continue;
        }

        let cells = split_table_row(line);
        if cells.iter().any(|cell| cell == "benchmark") {
            score_index = cells.iter().position(|cell| cell == "score_ratio");
            continue;
        }
        record_jetstream_row(&mut parsed, &cells, score_index);
    }

    if parsed.benchmark_count == 0 {
        parsed.benchmark_count = parsed.score_values.len();
    }
    parsed.score_below =
        summary_score_below.unwrap_or_else(|| count_below_budget(&parsed.score_values));
    parsed
}

fn record_jetstream_row(
    parsed: &mut ParsedJetStream,
    cells: &[String],
    score_index: Option<usize>,
) {
    if let Some(value) = score_index
        .and_then(|index| cells.get(index))
        .and_then(|cell| parse_ratio(cell))
    {
        parsed.score_values.push(value);
    }
}

fn parse_summary_count(line: &str, label: &str) -> Option<usize> {
    let suffix = line.trim().strip_prefix("- ")?;
    let value = suffix.strip_prefix(label)?.strip_prefix(": ")?;
    value.parse().ok()
}

fn split_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .map(str::to_owned)
        .collect()
}

fn parse_ratio(text: &str) -> Option<f64> {
    let ratio = text.trim().strip_suffix('x')?;
    ratio.parse().ok()
}

fn count_below_budget(values: &[f64]) -> usize {
    values.iter().filter(|value| **value < BUDGET_RATIO).count()
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

fn usize_to_f64(value: usize) -> Option<f64> {
    let value = u32::try_from(value).ok()?;
    Some(f64::from(value))
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, ensure};

    use super::parse_metrics;

    type TestResult = anyhow::Result<()>;

    #[test]
    fn parses_jetstream_ratios_from_ascii_table() -> TestResult {
        let text = "\
## JetStream Shell Benchmarks

- Measured: 2
- Below score budget (1.00x): 1

| benchmark | status | score_ratio |
| --- | --- | --- |
| hash-map | ✅ passed | 0.50x |
| tags | ✅ passed | 1.25x |
";

        let parsed = parse_metrics(text);
        ensure_usize(parsed.benchmark_count, 2)?;
        ensure_usize(parsed.score_below, 1)?;
        ensure_f64(
            parsed
                .score_geomean
                .context("JetStream geomean should be available")?,
            (0.50_f64 * 1.25_f64).sqrt(),
        )
    }

    #[test]
    fn derives_jetstream_counts_when_summary_is_absent() -> TestResult {
        let text = "\
## JetStream Shell Benchmarks

| benchmark | score_ratio |
| --- | --- |
| hash-map | 0.25x |
| tags | 2.00x |
";

        let parsed = parse_metrics(text);
        ensure_usize(parsed.benchmark_count, 2)?;
        ensure_usize(parsed.score_below, 1)?;
        ensure_f64(
            parsed
                .score_geomean
                .context("JetStream geomean should be available")?,
            (0.25_f64 * 2.00_f64).sqrt(),
        )
    }

    fn ensure_usize(actual: usize, expected: usize) -> TestResult {
        ensure!(
            actual == expected,
            "expected {expected} but received {actual}"
        );
        Ok(())
    }

    fn ensure_f64(actual: f64, expected: f64) -> TestResult {
        let difference = (actual - expected).abs();
        ensure!(
            difference < 0.000_001,
            "expected {expected} but received {actual}"
        );
        Ok(())
    }
}
