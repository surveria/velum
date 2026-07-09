use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_array_buffer_and_uint8_array_index_access() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let buffer = new ArrayBuffer(8);
        let data = new Uint8Array(buffer);
        data[0] = 257;
        data[1] = -1;
        data[2] = 42;
        data[20] = 99;

        print(
            buffer.byteLength,
            data.length,
            data.byteLength,
            data.byteOffset
        );
        print(data[0], data[1], data[2], data[20]);

        buffer.byteLength === 8 &&
            data.length === 8 &&
            data.byteLength === 8 &&
            data.byteOffset === 0 &&
            data[0] === 1 &&
            data[1] === 255 &&
            data[2] === 42 &&
            data[20] === undefined ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["8 8 8 0".to_owned(), "1 255 42 undefined".to_owned()],
    )
}

#[test]
fn exposes_host_provided_uint8_array_global() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.create_host_uint8_array_global("imageData", vec![0, 1, 2, 3])?;
    if !matches!(value, Value::Object(_)) {
        return Err("expected host array value to be an object".into());
    }
    ensure_optional_origin(context.typed_array_debug_origin(&value)?, "host-provided")?;

    let result = context.eval(
        r"
        imageData[0] = imageData[1] + imageData[2] + imageData[3];
        imageData[0]
        ",
    )?;
    ensure_value(&result, &Value::Number(6.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_optional_origin(actual: Option<&str>, expected: &str) -> TestResult {
    if actual == Some(expected) {
        return Ok(());
    }
    Err(format!("expected origin {expected:?}, got {actual:?}").into())
}
