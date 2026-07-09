use std::{
    fs,
    path::{Path, PathBuf},
};

const JETSTREAM_REPORT_PREFIX: &str = "rsqjs-jetstream-report-";
const JETSTREAM_REPORT_SUFFIX: &str = ".md";
const JETSTREAM_YAML_SUFFIX: &str = ".yaml";
const JETSTREAM_REPORT_DIR: &str = "jetstream-runs";
const JETSTREAM_SECTION: &str = "JetStream Shell Benchmarks";
const TEST_REPORT_PREFIX: &str = "rsqjs-test-report-";
const TEST_REPORT_SUFFIX: &str = ".md";
const BUDGET_RATIO: f64 = 1.00;

#[derive(Debug, Clone, Copy, Default)]
pub struct JetStreamMetrics {
    pub benchmark_count: usize,
    pub latency_geomean: Option<f64>,
    pub latency_over: usize,
}

pub fn structured_reports(reports_root: &Path) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let directory = reports_root.join(JETSTREAM_REPORT_DIR);
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut reports = Vec::new();
    for entry in fs::read_dir(&directory)? {
        let path = entry?.path();
        let Some(timestamp) = structured_report_timestamp(&path) else {
            continue;
        };
        reports.push((path, timestamp));
    }
    reports.sort_by(|left, right| left.1.cmp(&right.1));
    Ok(reports)
}

fn structured_report_timestamp(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let timestamp = file_name
        .strip_prefix(JETSTREAM_REPORT_PREFIX)?
        .strip_suffix(JETSTREAM_YAML_SUFFIX)?;
    canonical_timestamp(timestamp).then(|| timestamp.to_owned())
}

fn canonical_timestamp(timestamp: &str) -> bool {
    let bytes = timestamp.as_bytes();
    bytes.len() == 16
        && bytes.get(8) == Some(&b'T')
        && bytes.get(15) == Some(&b'Z')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 8 | 15) || byte.is_ascii_digit())
}

#[derive(Debug, Default)]
struct ParsedJetStream {
    benchmark_count: usize,
    latency_values: Vec<f64>,
    latency_over: usize,
}

#[derive(Debug, Clone, Copy)]
enum RatioColumn {
    Latency,
    LegacyScore,
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
        latency_geomean: geomean(&parsed.latency_values),
        latency_over: parsed.latency_over,
    }
}

fn parse_jetstream_metrics(text: &str) -> ParsedJetStream {
    let mut parsed = ParsedJetStream::default();
    let mut in_section = false;
    let mut ratio_column = None;
    let latency_summary_label = format!("Over latency budget ({})", super::BUDGET_LABEL);
    let legacy_score_summary_label = format!("Below score budget ({})", super::BUDGET_LABEL);
    let mut summary_latency_over = None;

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
        if let Some(count) = parse_summary_count(line, &latency_summary_label) {
            summary_latency_over = Some(count);
        }
        if let Some(count) = parse_summary_count(line, &legacy_score_summary_label) {
            summary_latency_over = Some(count);
        }
        if !line.starts_with('|') {
            continue;
        }

        let cells = split_table_row(line);
        if cells.iter().any(|cell| cell == "benchmark") {
            ratio_column = cells
                .iter()
                .position(|cell| cell == "latency_ratio")
                .map(|index| (index, RatioColumn::Latency))
                .or_else(|| {
                    cells
                        .iter()
                        .position(|cell| cell == "score_ratio")
                        .map(|index| (index, RatioColumn::LegacyScore))
                });
            continue;
        }
        record_jetstream_row(&mut parsed, &cells, ratio_column);
    }

    if parsed.benchmark_count == 0 {
        parsed.benchmark_count = parsed.latency_values.len();
    }
    parsed.latency_over =
        summary_latency_over.unwrap_or_else(|| count_over_budget(&parsed.latency_values));
    parsed
}

fn record_jetstream_row(
    parsed: &mut ParsedJetStream,
    cells: &[String],
    ratio_column: Option<(usize, RatioColumn)>,
) {
    let Some((index, kind)) = ratio_column else {
        return;
    };
    let Some(value) = cells.get(index).and_then(|cell| parse_ratio(cell)) else {
        return;
    };
    let Some(latency_ratio) = normalize_ratio(value, kind) else {
        return;
    };
    parsed.latency_values.push(latency_ratio);
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

fn normalize_ratio(value: f64, kind: RatioColumn) -> Option<f64> {
    match kind {
        RatioColumn::Latency => Some(value),
        RatioColumn::LegacyScore => {
            if value <= 0.0 {
                return None;
            }
            Some(1.0 / value)
        }
    }
}

fn count_over_budget(values: &[f64]) -> usize {
    values.iter().filter(|value| **value > BUDGET_RATIO).count()
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
    fn parses_jetstream_latency_ratios_from_ascii_table() -> TestResult {
        let text = "\
## JetStream Shell Benchmarks

- Measured: 2
- Over latency budget (1.00x): 1

| benchmark | status | latency_ratio |
| --- | --- | --- |
| hash-map | ✅ passed | 0.50x |
| tags | ✅ passed | 1.25x |
";

        let parsed = parse_metrics(text);
        ensure_usize(parsed.benchmark_count, 2)?;
        ensure_usize(parsed.latency_over, 1)?;
        ensure_f64(
            parsed
                .latency_geomean
                .context("JetStream geomean should be available")?,
            (0.50_f64 * 1.25_f64).sqrt(),
        )
    }

    #[test]
    fn converts_legacy_jetstream_score_ratios() -> TestResult {
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
        ensure_usize(parsed.latency_over, 1)?;
        ensure_f64(
            parsed
                .latency_geomean
                .context("JetStream geomean should be available")?,
            (2.00_f64 * 0.80_f64).sqrt(),
        )
    }

    #[test]
    fn derives_jetstream_counts_when_summary_is_absent() -> TestResult {
        let text = "\
## JetStream Shell Benchmarks

| benchmark | latency_ratio |
| --- | --- |
| hash-map | 0.25x |
| tags | 2.00x |
";

        let parsed = parse_metrics(text);
        ensure_usize(parsed.benchmark_count, 2)?;
        ensure_usize(parsed.latency_over, 1)?;
        ensure_f64(
            parsed
                .latency_geomean
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
