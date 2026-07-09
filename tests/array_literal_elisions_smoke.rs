use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_array_literal_elisions_as_holes() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let holes = [,, 42, ,];
        Array.prototype[0] = "proto";
        let inherited = holes[0];
        delete Array.prototype[0];
        let spread = [, ...[1, 2]];
        holes.length === 4 &&
            !Object.hasOwn(holes, "0") &&
            !Object.hasOwn(holes, "1") &&
            holes[2] === 42 &&
            !Object.hasOwn(holes, "3") &&
            inherited === "proto" &&
            spread.length === 3 &&
            !Object.hasOwn(spread, "0") &&
            spread[1] === 1 &&
            spread[2] === 2 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected {expected:?}, got {actual:?}").into())
}
