use velum::Runtime;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn assignment_to_const_in_tdz_throws_reference_error() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval("a = 3; const a = 1;") else {
        return Err("expected TDZ assignment to fail".into());
    };
    ensure_error_contains(&error, "ReferenceError")?;
    ensure_error_contains(&error, "not initialized")
}

fn ensure_error_contains(error: &velum::Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
