use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const ARRAY_SCAN_SCRIPT: &str = r#"
let object = { marker: 1 };
let packed = [object, "x", object, null, undefined, 3];

let packedOk =
    packed.includes(object) &&
    packed.indexOf(object) === 0 &&
    packed.lastIndexOf(object) === 2 &&
    packed.join("|") === "[object Object]|x|[object Object]|||3";

let holey = Array(5);
holey[1] = object;
holey[3] = "tail";

let holeyOk =
    holey.includes(undefined) &&
    holey.includes(object) &&
    holey.indexOf(object) === 1 &&
    holey.lastIndexOf(object) === 1 &&
    holey.join(",") === ",[object Object],,tail,";

packedOk && holeyOk ? 42 : 0
"#;

#[test]
fn array_scan_fast_paths_preserve_object_and_hole_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(ARRAY_SCAN_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
