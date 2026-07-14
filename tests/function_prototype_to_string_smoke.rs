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
        let sourceUnavailableCallables = [bound, proxy, hostNoop];
        let nativeSources = sourceUnavailableCallables.every(function (callable) {
            return source.call(callable) === "function () { [native code] }";
        });
        source.call(ordinary) === "function named() {}" && nativeSources &&
            source.call(Array) === "function Array() { [native code] }" &&
            source.call(Math.abs) === "function abs() { [native code] }" ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn retains_ordinary_function_and_computed_method_source() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        function check(expected, actual = new.target) {
            return actual === expected;
        }
        let restored = eval("(" + check.toString() + ")");
        let keyMethod = { a(){} }.a;
        let computed = { [keyMethod](){ return 42; } }["a(){}"];
        let commented = {
            [ /* a */ "f" /* b */ ] /* c */ ( /* d */ ) /* e */ { /* f */ }
        }.f;
        check.toString().includes("actual = new.target") &&
            new restored(restored) instanceof restored &&
            restored(undefined) &&
            keyMethod.toString() === "a(){}" &&
            computed() === 42 &&
            commented.toString() ===
                '[ /* a */ "f" /* b */ ] /* c */ ( /* d */ ) /* e */ { /* f */ }' ? 42 : 0;
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn retains_complete_generator_and_async_function_expression_source() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let generator = function /* g */ * /* n */ named() {};
        let asynchronous = async /* a */ function /* n */ named() {};
        let asyncGenerator = async /* a */ function /* g */ * named() {};
        generator.toString() === "function /* g */ * /* n */ named() {}" &&
            asynchronous.toString() === "async /* a */ function /* n */ named() {}" &&
            asyncGenerator.toString() ===
                "async /* a */ function /* g */ * named() {}" ? 42 : 0
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
