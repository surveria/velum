use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_promise_constructor_and_methods() -> TestResult {
    let value = eval(
        r"
        typeof Promise === 'function' &&
        Promise.name === 'Promise' &&
        Promise.length === 1 &&
        typeof Promise.resolve === 'function' &&
        Promise.resolve.length === 1 &&
        typeof Promise.reject === 'function' &&
        Promise.reject.length === 1 &&
        typeof Promise.prototype.then === 'function' &&
        Promise.prototype.then.length === 2 &&
        typeof Promise.prototype.catch === 'function' &&
        Promise.prototype.catch.length === 1 &&
        Promise.prototype.constructor === Promise
        ",
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn drains_resolved_promise_then_jobs_after_eval() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let value = 0;
        Promise.resolve(40).then(function(resolved) {
            value = resolved + 2;
        });
        ",
    )?;

    let value = context.eval("value")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn propagates_rejected_promise_to_catch_handler() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let reason = "";
        Promise.reject("offline").catch(function(error) {
            reason = error;
        });
        "#,
    )?;

    let value = context.eval("reason")?;
    ensure_value(&value, &Value::String("offline".to_owned()))
}

#[test]
fn async_function_returns_a_resolved_promise() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        async function answer() {
            return 42;
        }
        let value = 0;
        answer().then(function(resolved) {
            value = resolved;
        });
        ",
    )?;

    let value = context.eval("value")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn await_reads_already_resolved_promise_value() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        async function answer() {
            let base = await Promise.resolve(40);
            return base + 2;
        }
        let value = 0;
        answer().then(function(resolved) {
            value = resolved;
        });
        ",
    )?;

    let value = context.eval("value")?;
    ensure_value(&value, &Value::Number(42.0))
}

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
