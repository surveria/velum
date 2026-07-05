use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const MATH_RANDOM_SCRIPT: &str = r#"
let first = Math.random();
let second = Math.random();

let metadataOk =
    Math.random.name === "random" &&
    Math.random.length === 0;

let typeOk =
    typeof first === "number" &&
    typeof second === "number" &&
    first === first &&
    second === second;

let rangeOk =
    first >= 0 &&
    first < 1 &&
    second >= 0 &&
    second < 1;

print(metadataOk, typeOk, rangeOk);

metadataOk && typeOk && rangeOk ? 42 : 0
"#;

#[test]
fn exposes_per_vm_math_random() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(MATH_RANDOM_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["true true true"])?;

    let first_sequence_value = runtime.context().eval("Math.random()")?;
    let second_sequence_value = runtime.context().eval("Math.random()")?;
    ensure_value(&first_sequence_value, &second_sequence_value)
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
