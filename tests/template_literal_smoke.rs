use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn supports_no_substitution_template_literals() -> TestResult {
    let value = eval(
        r"
        let empty = ``;
        let text = `front`;
        let escaped = `\`\$\\`;
        let lines = `north
south`;
        empty + ':' + text + ':' + escaped + ':' + lines
        ",
    )?;

    ensure_value(
        &value,
        &Value::String(":front:`$\\:north\nsouth".to_owned()),
    )
}

#[test]
fn rejects_template_literal_substitutions() -> TestResult {
    ensure_error_contains(
        "`hello ${name}`",
        "template literal substitutions are not supported",
    )
}

#[test]
fn rejects_unterminated_template_literal() -> TestResult {
    ensure_error_contains("`unterminated", "unterminated template literal")
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
