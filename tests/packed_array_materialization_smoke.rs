use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const PACKED_ARRAY_MATERIALIZATION_SCRIPT: &str = r"
let object = { marker: 7 };
let values = [1, object, 3, 4];
let slice = values.slice(1, 3);
let concat = values.concat([5, object], 6);

let sliceOk =
    slice.length === 2 &&
    slice[0] === object &&
    slice[1] === 3;

let concatOk =
    concat.length === 7 &&
    concat[0] === 1 &&
    concat[1] === object &&
    concat[4] === 5 &&
    concat[5] === object &&
    concat[6] === 6;

sliceOk && concatOk ? 42 : 0
";

#[test]
fn packed_slice_and_concat_preserve_materialized_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(PACKED_ARRAY_MATERIALIZATION_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
