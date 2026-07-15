use velum::{Error, JsBigInt, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
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
fn supports_legacy_numeric_literals_only_in_sloppy_code() -> TestResult {
    let value = eval("[00, 01, 070, 077, 08, 09].join(',')")?;
    ensure_value(&value, &Value::String("0,1,56,63,8,9".to_owned().into()))?;

    ensure_error_contains(r#""use strict"; 01"#, "legacy numeric literal")?;
    ensure_error_contains(r#""use strict"; 08"#, "legacy numeric literal")
}

#[test]
fn rejects_invalid_numeric_literal_separators() -> TestResult {
    ensure_error_contains("1__0", "separator must be followed by a digit")?;
    ensure_error_contains("0x_FF", "misplaced numeric separator")?;
    ensure_error_contains(
        "1e_2",
        "decimal exponent literal has misplaced numeric separator",
    )?;
    ensure_error_contains("0_0", "cannot contain a numeric separator")
}

#[test]
fn rejects_identifier_immediately_after_numeric_literal() -> TestResult {
    ensure_error_contains("3in []", "cannot be immediately followed")?;
    ensure_error_contains("0x1g", "cannot be immediately followed")
}

#[test]
fn supports_bigint_literal_suffix() -> TestResult {
    let value = eval("9007199254740993n")?;
    ensure_value(
        &value,
        &Value::BigInt(JsBigInt::from_u64(9_007_199_254_740_993)),
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
