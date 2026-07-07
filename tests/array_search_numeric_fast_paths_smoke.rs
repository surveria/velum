use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const ARRAY_SEARCH_NUMERIC_SCRIPT: &str = r"
let values = [NaN, -0, 1, 2, 1];
let packedOk =
    values.includes(NaN) &&
    values.indexOf(NaN) === -1 &&
    values.includes(+0) &&
    values.indexOf(+0) === 1 &&
    values.lastIndexOf(1) === 4 &&
    values.lastIndexOf(1, 3) === 2;

let sparse = Array(4);
sparse[3] = 7;
let holeyOk =
    sparse.includes(undefined) &&
    sparse.includes(7) &&
    sparse.indexOf(undefined) === -1 &&
    sparse.indexOf(7) === 3 &&
    sparse.lastIndexOf(undefined) === -1 &&
    sparse.lastIndexOf(7) === 3;

packedOk && holeyOk ? 42 : 0
";

#[test]
fn array_numeric_search_fast_paths_preserve_js_equality_rules() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(ARRAY_SEARCH_NUMERIC_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
