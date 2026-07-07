use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const CALL_VALUE_INLINE_CACHE_SCRIPT: &str = r#"
var plusOne = function(value) {
    return value + 1;
};
var doubleValue = function(value) {
    return value * 2;
};
var identity = function(callback) {
    return callback;
};
var invoke = function(callback, value) {
    return identity(callback)(value);
};

var jsTotal = invoke(plusOne, 1) +
    invoke(plusOne, 2) +
    invoke(doubleValue, 3) +
    invoke(plusOne, 4);

var pickNative = function(useAbs) {
    return useAbs ? Math.abs : Math.max;
};
var nativeOrder = "";
var markNative = function(label, value) {
    nativeOrder = nativeOrder + label;
    return value;
};
var runNative = function(useAbs, left, right) {
    return pickNative(useAbs)(
        markNative(useAbs ? (left < -5 ? "a" : "c") : "e", left),
        markNative(useAbs ? (left < -5 ? "b" : "d") : "f", right)
    );
};
var nativeTotal = runNative(true, -7, 0) +
    runNative(true, -3, 0) +
    runNative(false, 2, 9);

var pickHost = function() {
    return hostAddOne;
};
var runHost = function(value) {
    return pickHost()(value);
};
var hostTotal = runHost(20) + runHost(21);

nativeOrder === "abcdef" ? jsTotal + nativeTotal + hostTotal : 0
"#;

#[test]
fn call_value_sites_cache_js_native_and_host_dispatch() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_host_function_typed("hostAddOne", |call| {
        let value: f64 = call.argument(0, "value")?;
        Ok(value + 1.0)
    })?;

    let script = vm.compile(CALL_VALUE_INLINE_CACHE_SCRIPT)?;
    ensure_at_least(
        script.usage().static_call_site_count(),
        14,
        "static call sites",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(78.0))?;

    let usage = vm.resource_usage();
    ensure_at_least(usage.call_value_cache_misses, 3, "call value cache misses")?;
    ensure_at_least(usage.call_value_cache_hits, 3, "call value cache hits")?;
    ensure_at_least(
        usage.call_value_cache_fallbacks,
        3,
        "call value cache fallbacks",
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }
    Err(format!("expected {label} >= {expected}, got {actual}").into())
}
