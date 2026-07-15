use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_async_arrow_expression_body() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let answer = async value => value + 1;
        let observed = 0;
        answer(41).then(function(resolved) {
            observed = resolved;
        });
        ",
    )?;

    let value = context.eval("observed")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_async_arrow_parenthesized_params_and_await() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let answer = async (left, right) => {
            let base = await Promise.resolve(left);
            return base + right;
        };
        let observed = 0;
        answer(40, 2).then(function(resolved) {
            observed = resolved;
        });
        ",
    )?;

    let value = context.eval("observed")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_async_arrow_default_parameters() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let answer = async (left = 40, right = 2,) => {
            let base = await Promise.resolve(left);
            return base + right;
        };
        let observed = 0;
        answer(undefined).then(function(resolved) {
            observed = resolved;
        });
        ",
    )?;

    let value = context.eval("observed")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_plain_arrow_callbacks_for_promise_then() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let observed = 0;
        Promise.resolve(41).then(value => {
            observed = value + 1;
        });
        ",
    )?;

    let value = context.eval("observed")?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
