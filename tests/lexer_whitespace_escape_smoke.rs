use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn treats_zero_width_no_break_space_as_ecmascript_whitespace() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval("/value/g\u{FEFF}; 42")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn braced_unicode_escapes_accept_unbounded_leading_zeros() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(r#""\u{0000000000000000000000000000000000000041}""#)?;
    ensure_value(&value, &Value::from("A"))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
