use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const NATIVE_CALL_SITE_CACHE_SCRIPT: &str = r"
let runAbs = function(value) {
    return Math.abs(value);
};

let firstAbs = runAbs(-7);
Math.abs = function(value) {
    return value + 100;
};
let secondAbs = runAbs(1);

let runPush = function(target, value) {
    return target.push(value);
};

let values = [];
let firstPush = runPush(values, 1);
Array.prototype.push = function(value) {
    this[0] = value + 10;
    return 99;
};
let secondPush = runPush(values, 2);

firstAbs === 7 &&
    secondAbs === 101 &&
    firstPush === 1 &&
    secondPush === 99 &&
    values[0] === 12 ? 42 : 0
";

#[test]
fn cached_native_call_sites_follow_property_mutations() -> TestResult {
    let runtime = Runtime::new();
    let script = runtime.compile(NATIVE_CALL_SITE_CACHE_SCRIPT)?;
    let mut context = runtime.context();

    let value = context.eval_compiled(&script)?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
