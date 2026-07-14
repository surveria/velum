use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const MATH_METHODS_SCRIPT: &str = r#"
let near = function(actual, expected) {
    return Math.abs(actual - expected) < 0.000001;
};

let metadataOk =
    Math.acos.name === "acos" &&
    Math.acos.length === 1 &&
    Math.atan2.name === "atan2" &&
    Math.atan2.length === 2 &&
    Math.hypot.name === "hypot" &&
    Math.hypot.length === 2;

let trigOk =
    near(Math.acos(1), 0) &&
    near(Math.asin(0), 0) &&
    near(Math.atan(1), Math.PI / 4) &&
    near(Math.atan2(1, 1), Math.PI / 4) &&
    near(Math.cos(0), 1) &&
    near(Math.sin(0), 0) &&
    near(Math.tan(0), 0);

let logOk =
    near(Math.exp(1), Math.E) &&
    near(Math.expm1(0), 0) &&
    near(Math.log(Math.E), 1) &&
    near(Math.log10(100), 2) &&
    near(Math.log1p(0), 0) &&
    near(Math.log2(8), 3);

let rootOk =
    near(Math.cbrt(27), 3) &&
    near(Math.cbrt(-8), -2) &&
    Math.sqrt(81) === 9;

let signOk =
    Math.sign(-2) === -1 &&
    Math.sign(2) === 1 &&
    Math.sign(0) === 0 &&
    1 / Math.sign(-0) === -Infinity;

let hyperOk =
    near(Math.sinh(0), 0) &&
    near(Math.cosh(0), 1) &&
    near(Math.tanh(0), 0) &&
    near(Math.asinh(0), 0) &&
    near(Math.acosh(1), 0) &&
    near(Math.atanh(0), 0) &&
    Math.abs(Math.atanh(-0.9999983310699463) - (-6.998237084679027)) < 0.00000000000001;

let hypotOk =
    Math.hypot() === 0 &&
    Math.hypot(3, 4) === 5 &&
    Math.hypot(Infinity, NaN) === Infinity &&
    Math.hypot(NaN, 3) !== Math.hypot(NaN, 3);

let nanOk =
    Math.acos() !== Math.acos() &&
    Math.log(-1) !== Math.log(-1) &&
    Math.sign(NaN) !== Math.sign(NaN);

let order = "";
let mark = function(label, value) {
    order = order + label;
    return value;
};
let fixedArgAbs = Math.abs(mark("a", -7), mark("b", 1)) === 7;
let fixedArgAtan2 = near(Math.atan2(mark("c", 1), mark("d", 1), mark("e", 1)), Math.PI / 4);
let fixedArgPow = Math.pow(mark("f", 2), mark("g", 5), mark("h", 0)) === 32;
let fixedArgRandom = typeof Math.random(mark("i", 0), mark("j", 0)) === "number";
let fixedArgEvaluationOk =
    fixedArgAbs &&
    fixedArgAtan2 &&
    fixedArgPow &&
    fixedArgRandom &&
    order === "abcdefghij";

print(metadataOk, trigOk, logOk);
print(rootOk, signOk, hyperOk, hypotOk, nanOk, fixedArgEvaluationOk);

metadataOk &&
    trigOk &&
    logOk &&
    rootOk &&
    signOk &&
    hyperOk &&
    hypotOk &&
    nanOk &&
    fixedArgEvaluationOk ? 42 : 0
"#;

#[test]
fn exposes_additional_math_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(MATH_METHODS_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["true true true", "true true true true true true"],
    )
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
