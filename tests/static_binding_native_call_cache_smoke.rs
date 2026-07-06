use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const STATIC_BINDING_NATIVE_CALL_CACHE_SCRIPT: &str = r#"
var call = function(fn, value) {
    return fn(value);
};

var firstNative = call(Boolean, 0) === false;
var custom = call(function(value) {
    return value + 40;
}, 2) === 42;
var secondNative = call(Number, "7") === 7;
var thirdNative = call(String, 8) === "8";
var fourthNative = call(Boolean, 1) === true;

firstNative &&
    custom &&
    secondNative &&
    thirdNative &&
    fourthNative ? 42 : 0
"#;

#[test]
fn cached_identifier_native_calls_follow_current_binding_value() -> TestResult {
    let runtime = Runtime::new();
    let script = runtime.compile(STATIC_BINDING_NATIVE_CALL_CACHE_SCRIPT)?;
    let mut context = runtime.context();

    let value = context.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let value = context.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
