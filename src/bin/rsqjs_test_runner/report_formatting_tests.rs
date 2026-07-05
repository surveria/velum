use super::{FeatureAreaStats, coverage_percent, feature_area_rows_with_limit};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn formats_small_coverage_with_four_decimals() -> TestResult {
    ensure_text(&coverage_percent(4, 53_683), "0.0074%")
}

#[test]
fn formats_feature_area_progress_row() -> TestResult {
    let mut stats = FeatureAreaStats::new("language/statements".to_owned());
    stats.record_manifest_enabled();
    stats.record_manifest_enabled();
    stats.record_passed();
    stats.record_failed();
    stats.record_skipped("not enabled yet: Test262 language cases".to_owned());
    stats.record_skipped("not enabled yet: Test262 language cases".to_owned());
    stats.record_skipped("requires async support".to_owned());

    let rows = feature_area_rows_with_limit(vec![stats], 10);
    let Some(row) = rows.first() else {
        return Err("expected one feature area row".into());
    };

    ensure_text(&row.feature_area, "language/statements")?;
    ensure_usize(row.total, 5)?;
    ensure_usize(row.executed, 2)?;
    ensure_text(&row.passed, "1 ✅ passed")?;
    ensure_text(&row.failed, "1 ❌ failed")?;
    ensure_text(&row.skipped, "3 🟡 skipped")?;
    ensure_text(&row.pass_rate, "50.00%")?;
    ensure_usize(row.manifest_enabled, 2)?;
    ensure_text(
        &row.top_skip_reason,
        "2: not enabled yet: Test262 language cases",
    )
}

#[test]
fn compacts_feature_area_rows_after_limit() -> TestResult {
    let mut first = FeatureAreaStats::new("built-ins/Array".to_owned());
    first.record_passed();
    first.record_passed();

    let mut second = FeatureAreaStats::new("language/expressions".to_owned());
    second.record_failed();

    let rows = feature_area_rows_with_limit(vec![second, first], 1);
    ensure_usize(rows.len(), 2)?;
    let Some(first_row) = rows.first() else {
        return Err("expected first feature row".into());
    };
    ensure_text(&first_row.feature_area, "built-ins/Array")?;
    let Some(other_row) = rows.last() else {
        return Err("expected compacted feature row".into());
    };
    ensure_text(&other_row.feature_area, "other feature areas")?;
    ensure_usize(other_row.total, 1)?;
    ensure_text(&other_row.failed, "1 ❌ failed")
}

fn ensure_text(actual: &str, expected: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected '{expected}', got '{actual}'").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
