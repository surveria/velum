use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const DIRECT_MATH_TARGET_ARGS_SCRIPT: &str = r#"
let order = "";
let mark = function(label, value) {
    order = order + label;
    return value;
};

let run = function() {
    return Math.abs(mark("a", -8), mark("b", 0)) +
        Math.pow(mark("c", 2), mark("d", 5), mark("e", 0)) +
        Math.max(mark("f", 1), mark("g", 3), mark("h", 2));
};

let first = run();
let second = run();

first === 43 &&
    second === 43 &&
    order === "abcdefghabcdefgh" ? 42 : 0
"#;

const DIRECT_MATH_INTEGER_TARGETS_SCRIPT: &str = r#"
let order = "";
let mark = function(label, value) {
    order = order + label;
    return value;
};

let run = function() {
    let numericOk =
        Math.clz32(mark("a", 16)) === 27 &&
        Math.imul(mark("b", 7), mark("c", 3)) === 21 &&
        Math.fround(mark("d", 0.5)) === 0.5;
    let fallbackOk =
        Math.clz32(mark("e", "0x10")) === 27 &&
        Math.imul(mark("f", "0x10"), mark("g", 2)) === 32 &&
        Math.fround(mark("h", "0.5")) === 0.5;
    return numericOk && fallbackOk;
};

let first = run();
let second = run();

first &&
    second &&
    order === "abcdefghabcdefgh" ? 42 : 0
"#;

#[test]
fn direct_math_targets_preserve_argument_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(DIRECT_MATH_TARGET_ARGS_SCRIPT)?;
    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        3,
        "direct Math native call operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let usage = vm.resource_usage();
    ensure_at_least(
        usage.native_call_cache_misses,
        3,
        "direct Math native call cache misses",
    )?;
    ensure_at_least(
        usage.native_call_cache_hits,
        3,
        "direct Math native call cache hits",
    )
}

#[test]
fn direct_math_integer_targets_preserve_numeric_and_fallback_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(DIRECT_MATH_INTEGER_TARGETS_SCRIPT)?;
    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        6,
        "direct integer Math native call operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let usage = vm.resource_usage();
    ensure_at_least(
        usage.native_call_cache_misses,
        6,
        "direct integer Math native call cache misses",
    )?;
    ensure_at_least(
        usage.native_call_cache_hits,
        6,
        "direct integer Math native call cache hits",
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
