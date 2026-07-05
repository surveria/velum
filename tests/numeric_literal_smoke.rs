use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn supports_numeric_literal_syntax() -> TestResult {
    let value = eval(
        r"
        let hex = 0x2a;
        let upperHex = 0X2A;
        let binary = 0b101010;
        let octal = 0o52;
        let separated = 1_000;
        let exponent = 1_2e3;
        let leading = .5e2;
        hex + upperHex + binary + octal + separated + exponent + leading
        ",
    )?;

    ensure_value(&value, &Value::Number(13_218.0))
}

#[test]
fn rejects_invalid_numeric_literal_separators() -> TestResult {
    ensure_error_contains("1__0", "separator must be followed by a digit")?;
    ensure_error_contains("0x_FF", "misplaced numeric separator")?;
    ensure_error_contains(
        "1e_2",
        "decimal exponent literal has misplaced numeric separator",
    )
}

#[test]
fn rejects_bigint_suffix_without_bigint_support() -> TestResult {
    ensure_error_contains(
        "1n",
        "decimal numeric literal cannot use BigInt suffix without BigInt support",
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let Err(error) = eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    error_contains(&error, expected)
}

fn error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
