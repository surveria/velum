use super::{parse_benchmark_metrics, parse_corpus_counts, parse_rollup_test262_counts};

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

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
