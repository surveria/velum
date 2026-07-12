use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn formats_source_unavailable_callables_as_native_functions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_function("hostNoop", |_call| Ok(Value::Undefined))?;
    let value = context.eval(
        r#"
        let source = Function.prototype.toString;
        let ordinary = function named() {};
        let bound = ordinary.bind(null);
        let proxy = new Proxy(ordinary, {});
        let callables = [ordinary, bound, proxy, hostNoop, Array, Math.abs];
        callables.every(function (callable) {
            return source.call(callable) === "function () { [native code] }";
        }) ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn preserves_retained_dynamic_function_source() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let generated = Function("value", "return value + 1;");
        let source = generated.toString();
        source !== "function () { [native code] }" &&
            source.includes("function anonymous") &&
            source.includes("return value + 1;") ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_non_callable_receivers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let failures = 0;
        for (let value of [undefined, null, 1, "x", {}, []]) {
            try {
                Function.prototype.toString.call(value);
            } catch (error) {
                if (error instanceof TypeError) {
                    failures = failures + 1;
                }
            }
        }
        failures === 6 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
