use rs_quickjs::{Error, Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> Result<Value, Error> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn treats_async_as_contextual_identifier_outside_async_forms() -> TestResult {
    let value = eval(
        r"
        let async = {
            value: 40,
            add(value) { return value + 2; }
        };
        async.add(async.value);
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_async_as_function_and_arrow_binding_name() -> TestResult {
    let value = eval(
        r"
        function async(value) {
            return value + 1;
        }
        let normalArrow = async => async + 1;
        normalArrow(async(40));
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn preserves_async_function_and_async_arrow_parsing() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        async function async(value) {
            return await value;
        }
        let arrow = async value => await async(value);
        let observed = 0;
        arrow(Promise.resolve(42)).then(value => {
            observed = value;
        });
        ",
    )?;

    let value = context.eval("observed")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn respects_line_terminators_after_bare_async_statement() -> TestResult {
    let value = eval(
        r"
        let async = 40
        async
        + 2
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_bare_async_without_statement_separator() -> TestResult {
    let error = eval("let async = 1; async class").err();
    let Some(error) = error else {
        return Err("expected parse error for bare async without statement separator".into());
    };
    ensure_error_contains(&error, "expected statement terminator")
}

#[test]
fn rejects_await_as_an_async_arrow_parameter() -> TestResult {
    for source in ["async await => 1", "async aw\\u0061it => 1"] {
        let Some(error) = eval(source).err() else {
            return Err(format!("expected '{source}' to fail").into());
        };
        if !matches!(error, Error::Parse { .. }) {
            return Err(format!("expected parse error, got '{error}'").into());
        }
    }
    Ok(())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(error: &Error, expected: &str) -> TestResult {
    let actual = error.to_string();
    if actual.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error containing '{expected}', got '{actual}'").into())
}
