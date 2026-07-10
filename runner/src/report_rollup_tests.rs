use std::{fs, path::PathBuf};

use super::{
    build_rollup, parse_benchmark_metrics, parse_corpus_counts, parse_rollup_test262_counts,
    render_markdown,
    report_rollup_chart::write_chart,
    report_rollup_timeline::{CommitTimeline, repository_root_for_test},
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn parses_benchmark_ratios_from_ascii_table() -> TestResult {
    let text = r"# rs-quickjs Test Report

## Benchmarks

- Measured: 2
- Over latency budget (legacy): 0
- Over memory budget (legacy): 0

```text
| benchmark | latency_ratio | latency_budget | memory_ratio | memory_budget |
| alpha | 1.21x | 🟡 > 1.00x | 0.98x | ✅ <= 1.00x |
| beta | 1.00x | ✅ <= 1.00x | 1.11x | 🟡 > 1.00x |
```
";
    let parsed = parse_benchmark_metrics(text);
    ensure_usize(parsed.benchmark_count, 2)?;
    ensure_usize(parsed.latency_over, 1)?;
    ensure_usize(parsed.memory_over, 1)?;
    ensure_usize(parsed.latency_values.len(), 2)?;
    ensure_usize(parsed.memory_values.len(), 2)
}

#[test]
fn prefers_current_budget_summary_counts() -> TestResult {
    let text = r"# rs-quickjs Test Report

## Benchmarks

- Measured: 2
- Over latency budget (1.00x): 2
- Over memory budget (1.00x): 1

```text
| benchmark | latency_ratio | latency_budget | memory_ratio | memory_budget |
| alpha | 1.00x | 🟡 > 1.00x | 1.00x | 🟡 > 1.00x |
| beta | 1.00x | 🟡 > 1.00x | 1.00x | ✅ <= 1.00x |
```
";
    let parsed = parse_benchmark_metrics(text);
    ensure_usize(parsed.benchmark_count, 2)?;
    ensure_usize(parsed.latency_over, 2)?;
    ensure_usize(parsed.memory_over, 1)
}

#[test]
fn parses_full_test262_counts() -> TestResult {
    let text = r"# rs-quickjs Test Report

## Test262 full corpus

- Total: 100
- Passed: 25
- Failed: 75
";
    let Some(value) = parse_corpus_counts(text, "Test262 full corpus") else {
        return Err("expected corpus counts".into());
    };
    ensure_usize(usize::try_from(value.total)?, 100)?;
    ensure_usize(usize::try_from(value.passed)?, 25)?;
    ensure_usize(usize::try_from(value.failed)?, 75)
}

#[test]
fn rollup_prefers_variant_level_test262_counts() -> TestResult {
    let text = r"# rs-quickjs Test Report

## Test262 file conformance

- Total: 53404
- Passed: 15315
- Failed: 38089

## Test262 full corpus

- Total: 102578
- Passed: 29396
- Failed: 73182
";
    let Some(value) = parse_rollup_test262_counts(text) else {
        return Err("expected rollup Test262 counts".into());
    };
    ensure_usize(usize::try_from(value.total)?, 102_578)?;
    ensure_usize(usize::try_from(value.passed)?, 29_396)?;
    ensure_usize(usize::try_from(value.failed)?, 73_182)
}

#[test]
fn mixed_history_uses_yaml_for_new_reports_and_markdown_fallback_for_old_reports() -> TestResult {
    let root = temporary_report_root();
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    let report_dir = root.join("test-runs");
    fs::create_dir_all(&report_dir)?;
    let legacy_name = "rsqjs-test-report-20260708T000000Z.md";
    fs::write(
        report_dir.join(legacy_name),
        "# rs-quickjs Test Report\n\n## Benchmarks\n\n- Measured: 1\n- Over latency budget (1.00x): 0\n- Over memory budget (1.00x): 0\n\n| benchmark | latency_ratio | memory_ratio |\n| old | 1.10x | - |\n",
    )?;

    let current_name = "rsqjs-test-report-20260709T000000Z.md";
    fs::write(report_dir.join(current_name), "derived view")?;
    let summary = crate::report_schema_tests::sample_document()?.summary();
    let yaml = serde_yaml_ng::to_string(&summary)?;
    fs::write(
        report_dir.join("rsqjs-test-report-20260709T000000Z.yaml"),
        yaml,
    )?;

    let rollup = build_rollup(&report_dir)?;
    let valid =
        rollup.records.len() == 2
            && rollup.records.first().is_some_and(|record| {
                record.file_name == legacy_name && record.benchmark_count == 1
            })
            && rollup.records.get(1).is_some_and(|record| {
                record.file_name == current_name && record.benchmark_count == 1
            });
    fs::remove_dir_all(&root)?;
    if valid {
        return Ok(());
    }
    Err("mixed rollup history did not preserve both parser paths".into())
}

#[test]
fn standalone_jetstream_yaml_with_an_unrelated_timestamp_is_discovered_once() -> TestResult {
    let root = std::env::temp_dir().join(format!(
        "rsqjs-rollup-standalone-jetstream-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    let report_dir = root.join("test-runs");
    let jetstream_dir = root.join("jetstream-runs");
    fs::create_dir_all(&report_dir)?;
    fs::create_dir_all(&jetstream_dir)?;

    let test_name = "rsqjs-test-report-20260709T000000Z.md";
    fs::write(report_dir.join(test_name), "derived test report")?;
    fs::write(
        report_dir.join("rsqjs-test-report-20260709T000000Z.yaml"),
        serde_yaml_ng::to_string(&crate::report_schema_tests::sample_document()?.summary())?,
    )?;

    let jetstream_stem = "rsqjs-jetstream-report-20260710T031700Z";
    let jetstream_yaml = serde_yaml_ng::to_string(
        &crate::report_schema_tests::sample_jetstream_document()?.summary(),
    )?;
    fs::write(
        jetstream_dir.join(format!("{jetstream_stem}.yaml")),
        &jetstream_yaml,
    )?;
    fs::write(
        jetstream_dir.join(format!("{jetstream_stem}-component.yaml")),
        &jetstream_yaml,
    )?;
    fs::write(
        jetstream_dir.join(format!("{jetstream_stem}-exhaustive.yaml")),
        &jetstream_yaml,
    )?;
    fs::write(
        jetstream_dir.join(format!("{jetstream_stem}.md")),
        "derived JetStream report",
    )?;

    let rollup = build_rollup(&report_dir)?;
    let markdown = render_markdown(&rollup.records);
    let valid = rollup.records.len() == 2
        && rollup
            .records
            .first()
            .is_some_and(|record| record.file_name == test_name)
        && rollup.records.get(1).is_some_and(|record| {
            record.file_name == format!("{jetstream_stem}.yaml")
                && record.jetstream_count == 86
                && record.benchmark_count == 0
        })
        && markdown.contains(&format!(
            "- JetStream: 0.80x (0/86 >1.00x) (from `{jetstream_stem}.yaml`)"
        ))
        && markdown.contains(&format!(
            "- Performance: 1.25x (1/1 >1.00x) (from `{test_name}`)"
        ));
    fs::remove_dir_all(&root)?;
    if valid {
        return Ok(());
    }
    Err("standalone JetStream YAML was missing, duplicated, or misattributed".into())
}

#[test]
fn current_history_chart_renders_on_the_shared_main_commit_axis() -> TestResult {
    let repository_root = repository_root_for_test()?;
    let report_dir = repository_root.join("reports/test-runs");
    let records = super::parse_records(&report_dir)?;
    let timeline = CommitTimeline::discover(&report_dir, &records)?;
    let chart_path = std::env::temp_dir().join(format!(
        "rsqjs-commit-axis-chart-{}.jpg",
        std::process::id()
    ));
    write_chart(&records, &timeline, &chart_path)?;
    let metadata = fs::metadata(&chart_path)?;
    fs::remove_file(&chart_path)?;
    if metadata.len() > 100_000 && timeline.axis_end()? > i32::try_from(records.len())? {
        return Ok(());
    }
    Err("shared main commit chart was empty or used a compressed report domain".into())
}

fn temporary_report_root() -> PathBuf {
    std::env::temp_dir().join(format!("rsqjs-rollup-mixed-{}", std::process::id()))
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
