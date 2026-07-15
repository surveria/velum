use velum::{Context, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const HOST_NOOP_NAME: &str = "hostNoop";

#[test]
fn validates_all_current_object_like_storage_owners() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let ordinary = context.eval("({ marker: true })")?;
    let function = context.eval("(function sample() { return 1; })")?;
    let native = context.eval("Object")?;
    context.register_host_function(HOST_NOOP_NAME, |_call| Ok(Value::Undefined))?;
    let host = required_global(&context, HOST_NOOP_NAME)?;
    let error = context.eval("new TypeError('boom')")?;
    let typed = context.create_host_uint8_array_global("imageData", vec![1, 2, 3])?;

    for (label, value) in [
        ("ordinary object", ordinary),
        ("JavaScript function", function),
        ("native function", native),
        ("host function", host),
        ("error object", error),
    ] {
        ensure_origin(&context, &value, None, label)?;
    }
    ensure_origin(&context, &typed, Some("host-provided"), "typed array")?;
    ensure_origin(&context, &Value::Number(42.0), None, "primitive")
}

#[test]
fn rejects_object_like_values_whose_slots_are_not_defined() -> TestResult {
    let runtime = Runtime::new();
    let mut source = runtime.context();
    let ordinary = source.eval("({ marker: true })")?;
    let function = source.eval("(function sample() { return 1; })")?;
    let native = source.eval("Object")?;
    source.register_host_function(HOST_NOOP_NAME, |_call| Ok(Value::Undefined))?;
    let host = required_global(&source, HOST_NOOP_NAME)?;

    let target = runtime.context();
    ensure_storage_error(&target, &ordinary, "object id is not defined")?;
    ensure_storage_error(&target, &function, "function id is not defined")?;
    ensure_storage_error(&target, &native, "native function id is not defined")?;
    ensure_storage_error(&target, &host, "host function id is not defined")
}

#[test]
fn preserves_object_like_constructor_and_proxy_results() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_function(HOST_NOOP_NAME, |_call| Ok(Value::Number(42.0)))?;

    let value = context.eval(
        r#"
        let ErrorResult = function ErrorResult() {
            return new TypeError("constructor result");
        };
        let hostProxy = new Proxy(hostNoop, {});
        let constructed = new ErrorResult();

        constructed instanceof TypeError &&
            constructed.message === "constructor result" &&
            hostProxy() === 42 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

fn required_global(context: &Context, name: &str) -> TestResultValue {
    context
        .get_global(name)
        .ok_or_else(|| format!("expected global '{name}'").into())
}

type TestResultValue = std::result::Result<Value, Box<dyn std::error::Error>>;

fn ensure_origin(
    context: &Context,
    value: &Value,
    expected: Option<&str>,
    label: &str,
) -> TestResult {
    let actual = context.typed_array_debug_origin(value)?;
    if actual == expected {
        return Ok(());
    }
    Err(format!("{label}: expected origin {expected:?}, got {actual:?}").into())
}

fn ensure_storage_error(context: &Context, value: &Value, expected: &str) -> TestResult {
    match context.typed_array_debug_origin(value) {
        Err(error) if error.to_string().contains(expected) => Ok(()),
        Err(error) => Err(format!("expected error containing '{expected}', got '{error}'").into()),
        Ok(origin) => Err(format!("expected storage error, got origin {origin:?}").into()),
    }
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
