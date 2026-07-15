use velum::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
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

    let Err(error) = eval(r#""use strict"; "\09""#) else {
        return Err("expected strict legacy octal escape to fail".into());
    };
    ensure_error_contains(&error, "legacy escape sequence")
}

#[test]
fn supports_legacy_string_escapes_in_sloppy_code() -> TestResult {
    let value = eval(r#"["\0", "\1", "\10", "\377", "\400", "\8", "\9"].join(":")"#)?;
    ensure_value(
        &value,
        &Value::String("\0:\u{0001}:\u{0008}:ÿ: 0:8:9".to_owned().into()),
    )
}

#[test]
fn validates_legacy_escapes_across_strict_directive_prologues() -> TestResult {
    ensure_value(
        &eval(r#""use strict"; "\0""#)?,
        &Value::String("\0".to_owned().into()),
    )?;

    let Err(error) = eval(r#"function invalid() { "\1"; "use strict"; }"#) else {
        return Err("expected a preceding legacy escape in a strict prologue to fail".into());
    };
    ensure_error_contains(&error, "strict directive prologue")
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
