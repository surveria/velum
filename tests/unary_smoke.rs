use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_typeof_void_and_delete_unary_operators() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let camera = { name: "front-door", active: true };
        let values = [40, 2];
        let side = 0;

        let erasedName = delete camera.name;
        let erasedMissing = delete camera.missing;
        let erasedIndex = delete values[0];
        let erasedLength = delete values.length;
        let erasedBinding = delete side;
        let erasedUnknown = delete missingBinding;
        let voidValue = void (side = 42);
        let typeReport =
            typeof camera + " " +
            typeof camera.name + " " +
            typeof values[0] + " " +
            typeof missingBinding + " " +
            typeof function() {};

        print(erasedName, erasedMissing, erasedIndex, erasedLength, erasedBinding, erasedUnknown);
        print(typeReport);
        print(values.length, side, voidValue);

        camera.name === undefined &&
            values[0] === undefined &&
            values.length === 2 &&
            side === 42 &&
            voidValue === undefined &&
            typeReport === "object undefined undefined undefined function" &&
            erasedName === true &&
            erasedMissing === true &&
            erasedIndex === true &&
            erasedLength === false &&
            erasedBinding === false &&
            erasedUnknown === true
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(
        context.output(),
        &[
            "true true true false false true".to_owned(),
            "object undefined undefined undefined function".to_owned(),
            "2 42 undefined".to_owned(),
        ],
    )
}

#[test]
fn rejects_delete_property_on_nullish_base() -> TestResult {
    ensure_runtime_error_contains("delete undefined.field", "Cannot convert undefined or null")?;
    ensure_runtime_error_contains("delete null.field", "Cannot convert undefined or null")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_runtime_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };

    ensure_error_contains(&error, expected)
}

fn ensure_error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
