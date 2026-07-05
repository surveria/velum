use std::time::Duration;

use super::{coverage_percent, ratio};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn formats_ratio_below_one() -> TestResult {
    ensure_text(
        &ratio(Duration::from_micros(5), Duration::from_micros(366)),
        "0.01x",
    )
}

#[test]
fn formats_ratio_above_one() -> TestResult {
    ensure_text(
        &ratio(Duration::from_micros(250), Duration::from_micros(100)),
        "2.50x",
    )
}

#[test]
fn formats_small_coverage_with_four_decimals() -> TestResult {
    ensure_text(&coverage_percent(4, 53_683), "0.0074%")
}

fn ensure_text(actual: &str, expected: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected '{expected}', got '{actual}'").into())
}
