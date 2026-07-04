use rs_quickjs::{Error, Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn evaluates_arithmetic_with_precedence() -> TestResult {
    expect_value("1 + 2 * 3 - 4 / 2", &Value::Number(5.0))
}

#[test]
fn evaluates_bindings_and_assignment() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval("let x = 40; x = x + 2; x")?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("x"), &Value::Number(42.0))?;
    Ok(())
}

#[test]
fn keeps_const_bindings_immutable() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval("const x = 1; x = 2") else {
        return Err("expected const assignment to fail".into());
    };
    ensure_error_kind(&error, "runtime")?;
    Ok(())
}

#[test]
fn supports_strings_and_host_print() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(r#"let name = "camera"; print("hello", name); "id-" + 7"#)?;

    ensure_value(&value, &Value::String("id-7".to_owned()))?;
    ensure_output(context.output(), &["hello camera".to_owned()])?;
    Ok(())
}

#[test]
fn short_circuits_logical_operators() -> TestResult {
    expect_value("false && missing", &Value::Bool(false))?;
    expect_value(r#""ok" || missing"#, &Value::String("ok".to_owned()))
}

#[test]
fn enforces_resource_limits() -> TestResult {
    let limits = RuntimeLimits {
        max_source_len: 8,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let Err(error) = context.eval("let x = 10;") else {
        return Err("expected resource limit to fail".into());
    };
    ensure_error_kind(&error, "resource limit")?;
    Ok(())
}

fn expect_value(source: &str, expected: &Value) -> TestResult {
    let actual = eval(source)?;
    ensure_value(&actual, expected)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_optional_value(actual: Option<&Value>, expected: &Value) -> TestResult {
    if actual == Some(expected) {
        return Ok(());
    }

    Err(format!("expected global value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_error_kind(error: &Error, expected: &str) -> TestResult {
    let matches = matches!(
        (error, expected),
        (Error::Runtime { .. }, "runtime") | (Error::ResourceLimit { .. }, "resource limit")
    );

    if matches {
        return Ok(());
    }

    Err(format!("expected {expected} error, got {error:?}").into())
}
