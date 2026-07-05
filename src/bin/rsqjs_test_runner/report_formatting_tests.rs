use super::coverage_percent;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

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
