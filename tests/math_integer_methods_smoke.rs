use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const MATH_INTEGER_METHODS_SCRIPT: &str = r#"
let metadataOk =
    Math.clz32.name === "clz32" &&
    Math.clz32.length === 1 &&
    Math.fround.name === "fround" &&
    Math.fround.length === 1 &&
    Math.imul.name === "imul" &&
    Math.imul.length === 2;

let clzOk =
    Math.clz32(0) === 32 &&
    Math.clz32(-0) === 32 &&
    Math.clz32(1) === 31 &&
    Math.clz32(2147483648) === 0 &&
    Math.clz32(4294967296) === 32 &&
    Math.clz32(-4294967297) === 0 &&
    Math.clz32(NaN) === 32 &&
    Math.clz32(Infinity) === 32 &&
    Math.clz32("0x10") === 27;

let imulOk =
    Math.imul(2, 4) === 8 &&
    Math.imul(-1, 8) === -8 &&
    Math.imul(0xffffffff, 5) === -5 &&
    Math.imul(65535, 65535) === -131071 &&
    Math.imul(1.9, 7) === 7 &&
    Math.imul(7) === 0 &&
    Math.imul("0x10", 2) === 32;

let froundOk =
    Math.fround(0.1) === 0.10000000149011612 &&
    Math.fround(4294967295) === 4294967296 &&
    Math.fround(NaN) !== Math.fround(NaN) &&
    Math.fround(Infinity) === Infinity &&
    1 / Math.fround(-0) === -Infinity &&
    Math.fround("0.5") === 0.5;

let froundTieOk =
    Math.fround(1.0000000596046448) === 1 &&
    Math.fround(1.0000001788139343) === 1.000000238418579;

print(metadataOk, clzOk, imulOk);
print(froundOk, froundTieOk);

metadataOk && clzOk && imulOk && froundOk && froundTieOk ? 42 : 0
"#;

#[test]
fn exposes_integer_and_float32_math_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(MATH_INTEGER_METHODS_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["true true true", "true true"])
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
