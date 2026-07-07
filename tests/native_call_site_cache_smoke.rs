use rs_quickjs::{Engine, Runtime, Value};

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

const NATIVE_CALL_SITE_COUNTER_SCRIPT: &str = r#"
var runNative = function(values, marker) {
    return Math.abs(-4) + Math["max"](1, 2) + values.push(marker);
};

var values = [];
var first = runNative(values, 1);
var second = runNative(values, 2);

Math.abs = function(value) {
    return value + 100;
};
Array.prototype.push = function(value) {
    this[0] = value + 10;
    return 77;
};

var third = runNative(values, 3);
first === 7 &&
    second === 8 &&
    third === 175 &&
    values[0] === 13 ? 42 : 0
"#;

const DYNAMIC_NATIVE_MEMBER_CACHE_SCRIPT: &str = r#"
var method = "abs";
var order = "";
var mark = function(label, value) {
    order = order + label;
    return value;
};

var runUnary = function(value) {
    return Math[method](value);
};

var first = runUnary(mark("a", -3));
var second = runUnary(mark("b", -4));

Math.abs = function(value) {
    return value + 100;
};

var third = runUnary(5);

method = "max";
var fourth = Math[method](mark("c", 2), mark("d", 9));

first === 3 &&
    second === 4 &&
    third === 105 &&
    fourth === 9 &&
    order === "abcd" ? 42 : 0
"#;

#[test]
fn cached_native_call_sites_follow_property_mutations() -> TestResult {
    let runtime = Runtime::new();
    let script = runtime.compile(NATIVE_CALL_SITE_CACHE_SCRIPT)?;
    let mut context = runtime.context();

    let value = context.eval_compiled(&script)?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn native_call_site_cache_reports_hits_misses_and_slow_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(NATIVE_CALL_SITE_COUNTER_SCRIPT)?;
    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        3,
        "direct native call operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let usage = vm.resource_usage();
    ensure_at_least(
        usage.native_call_cache_misses,
        3,
        "native call cache misses",
    )?;
    ensure_at_least(usage.native_call_cache_hits, 3, "native call cache hits")?;
    ensure_at_least(
        usage.native_call_cache_slow_paths,
        2,
        "native call cache slow paths",
    )
}

#[test]
fn dynamic_native_member_call_sites_cache_object_property_hits() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(DYNAMIC_NATIVE_MEMBER_CACHE_SCRIPT)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let usage = vm.resource_usage();
    ensure_at_least(usage.native_call_cache_hits, 1, "native call cache hits")?;
    ensure_at_least(
        usage.native_call_cache_misses,
        2,
        "native call cache misses",
    )?;
    ensure_at_least(
        usage.native_call_cache_slow_paths,
        1,
        "native call cache slow paths",
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
