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

#[test]
fn supports_numeric_element_kinds_and_array_like_sources() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let i8 = new Int8Array([127, 128, -129]);
        let clamped = new Uint8ClampedArray([-1, 0.5, 1.5, 254.6, 300]);
        let i16 = new Int16Array([32767, 32768, -32769]);
        let u16 = new Uint16Array([-1, 65537]);
        let i32 = new Int32Array([2147483648, -2147483649]);
        let u32 = new Uint32Array([-1, 4294967297]);
        let f32 = new Float32Array([1.337, Infinity, NaN]);
        let f64 = new Float64Array([Math.PI, -0]);

        print(i8[0], i8[1], i8[2]);
        print(clamped[0], clamped[1], clamped[2], clamped[3], clamped[4]);
        print(i16[0], i16[1], i16[2], u16[0], u16[1]);
        print(i32[0], i32[1], u32[0], u32[1]);

        i8[0] === 127 && i8[1] === -128 && i8[2] === 127 &&
            clamped[0] === 0 && clamped[1] === 0 && clamped[2] === 2 &&
            clamped[3] === 255 && clamped[4] === 255 &&
            i16[0] === 32767 && i16[1] === -32768 && i16[2] === 32767 &&
            u16[0] === 65535 && u16[1] === 1 &&
            i32[0] === -2147483648 && i32[1] === 2147483647 &&
            u32[0] === 4294967295 && u32[1] === 1 &&
            f32[0] === Math.fround(1.337) && f32[1] === Infinity &&
            Number.isNaN(f32[2]) && f64[0] === Math.PI &&
            1 / f64[1] === -Infinity ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "127 -128 127".to_owned(),
            "0 0 2 255 255".to_owned(),
            "32767 -32768 32767 65535 1".to_owned(),
            "-2147483648 2147483647 4294967295 1".to_owned(),
        ],
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_shared_array_buffer_views_and_constructor_metadata() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let buffer = new ArrayBuffer(24);
        let u8 = new Uint8Array(buffer);
        let i16 = new Int16Array(buffer, 2, 3);
        let f64 = new Float64Array(buffer, 8, 2);

        i16[0] = -2;
        f64[0] = Math.PI;

        let names = [
            Int8Array, Uint8Array, Uint8ClampedArray,
            Int16Array, Uint16Array, Int32Array,
            Uint32Array, Float32Array, Float64Array
        ];
        let metadata = names[0].name === "Int8Array" &&
            names[8].name === "Float64Array" &&
            names[0].length === 1 && names[8].length === 1 &&
            Int16Array.BYTES_PER_ELEMENT === 2 &&
            Float64Array.BYTES_PER_ELEMENT === 8 &&
            Int16Array.prototype.BYTES_PER_ELEMENT === 2 &&
            ArrayBuffer.prototype.resize === undefined;

        print(buffer.byteLength, i16.length, i16.byteLength, i16.byteOffset);
        print(f64.length, f64.byteLength, f64.byteOffset);

        buffer.byteLength === 24 && i16.length === 3 && i16.byteLength === 6 &&
            i16.byteOffset === 2 && f64.length === 2 && f64.byteLength === 16 &&
            f64.byteOffset === 8 &&
            u8[2] === 254 && u8[3] === 255 && f64[0] === Math.PI && metadata ? 42 : 0
        "#,
    )?;

    ensure_output(
        context.output(),
        &["24 3 6 2".to_owned(), "2 16 8".to_owned()],
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_misaligned_views_and_calls_without_new() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let failures = 0;
        try { new Int16Array(new ArrayBuffer(8), 1); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { new Float64Array(new ArrayBuffer(8), 0, 2); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { Int8Array(1); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        try { ArrayBuffer(1); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        failures
        ",
    )?;

    ensure_value(&value, &Value::Number(4.0))
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
