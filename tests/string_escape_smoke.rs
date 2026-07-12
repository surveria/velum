use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn supports_common_string_escape_sequences() -> TestResult {
    let value = eval(
        r#"
        let simple = "\b\f\n\r\t\v\0";
        let hex = "\x41\u0042\u{43}";
        let quoted = "\"\'\\";
        let identity = "\a\c\-";
        let continuation = "front\
door";
        simple + ":" + hex + ":" + quoted + ":" + identity + ":" + continuation
        "#,
    )?;

    ensure_value(
        &value,
        &Value::String(
            "\u{0008}\u{000c}\n\r\t\u{000b}\0:ABC:\"'\\:ac-:frontdoor"
                .to_owned()
                .into(),
        ),
    )
}

#[test]
fn reports_invalid_escape_sequences() -> TestResult {
    let Err(error) = eval(r#""\xG0""#) else {
        return Err("expected invalid hex escape to fail".into());
    };
    ensure_error_contains(&error, "hex escape has non-hex digit")?;

    let Err(error) = eval(r#""\u{}""#) else {
        return Err("expected empty braced unicode escape to fail".into());
    };
    ensure_error_contains(&error, "empty braced unicode escape")?;

    let Err(error) = eval(r#""\09""#) else {
        return Err("expected legacy octal escape to fail".into());
    };
    ensure_error_contains(&error, "legacy octal escape sequences")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected '{message}' to contain '{expected}'").into())
}
