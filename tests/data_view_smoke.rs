use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_data_view_constructor_and_metadata() -> TestResult {
    ensure_eval(
        r#"
        let buffer = new ArrayBuffer(12);
        let view = new DataView(buffer, 2, 8);
        let descriptor = Object.getOwnPropertyDescriptor(DataView.prototype, "byteLength");
        typeof DataView === "function" &&
            DataView.name === "DataView" && DataView.length === 1 &&
            view.buffer === buffer && view.byteOffset === 2 && view.byteLength === 8 &&
            view.constructor === DataView &&
            Object.getPrototypeOf(view) === DataView.prototype &&
            typeof descriptor.get === "function" && descriptor.set === undefined &&
            descriptor.enumerable === false && descriptor.configurable === true &&
            DataView.prototype[Symbol.toStringTag] === "DataView" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn reads_and_writes_numeric_values_with_explicit_endianness() -> TestResult {
    ensure_eval(
        r"
        let buffer = new ArrayBuffer(32);
        let bytes = new Uint8Array(buffer);
        let view = new DataView(buffer, 4, 24);

        view.setInt16(0, -2);
        view.setUint16(2, 0x1234, true);
        view.setInt32(4, -2147483648, true);
        view.setUint32(8, 0x89abcdef);
        view.setFloat32(12, 1.5, true);
        view.setFloat64(16, -Math.PI);

        view.getInt16(0) === -2 && bytes[4] === 255 && bytes[5] === 254 &&
            view.getUint16(2, true) === 0x1234 && bytes[6] === 0x34 && bytes[7] === 0x12 &&
            view.getInt32(4, true) === -2147483648 &&
            view.getUint32(8) === 0x89abcdef &&
            view.getFloat32(12, true) === 1.5 &&
            view.getFloat64(16) === -Math.PI ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn applies_integer_conversion_and_float16_rounding() -> TestResult {
    ensure_eval(
        r"
        let view = new DataView(new ArrayBuffer(16));
        view.setInt8(0, 255);
        view.setUint8(1, -1);
        view.setInt16(2, 65535);
        view.setUint16(4, -1);
        view.setFloat16(6, 1.337, true);
        view.setFloat32(8, NaN);
        view.setFloat64(8, -0);

        view.getInt8(0) === -1 && view.getUint8(1) === 255 &&
            view.getInt16(2) === -1 && view.getUint16(4) === 65535 &&
            view.getFloat16(6, true) === Math.f16round(1.337) &&
            1 / view.getFloat64(8) === -Infinity ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn reads_and_writes_bigint_values_with_exact_endianness() -> TestResult {
    ensure_eval(
        r"
        let buffer = new ArrayBuffer(24);
        let view = new DataView(buffer, 4, 16);
        let bytes = new Uint8Array(buffer);
        view.setBigUint64(0, 0x0123456789abcdefn);
        view.setBigInt64(8, { valueOf() { return -2n; } }, true);

        let failures = 0;
        try { view.setBigInt64(0, 1); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        try { view.setUint32(0, 1n); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }

        view.getBigUint64(0) === 0x0123456789abcdefn &&
            bytes[4] === 0x01 && bytes[11] === 0xef &&
            view.getBigInt64(8, true) === -2n &&
            bytes[12] === 0xfe && bytes[19] === 0xff &&
            DataView.prototype.getBigInt64.length === 1 &&
            DataView.prototype.setBigUint64.length === 2 &&
            failures === 2 ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn validates_constructor_ranges_receivers_and_method_offsets() -> TestResult {
    ensure_eval(
        r"
        let failures = 0;
        let buffer = new ArrayBuffer(4);
        let view = new DataView(buffer, 1, 2);

        try { DataView(buffer); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        try { new DataView({}, 0); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        try { new DataView(buffer, 5); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { new DataView(buffer, 3, 2); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { view.getUint16(1); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { DataView.prototype.getUint8.call({}); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        failures
        ",
        &Value::Number(6.0),
    )
}

#[test]
fn tracks_resizable_buffer_length_and_revalidates_after_prototype_lookup() -> TestResult {
    ensure_eval(
        r#"
        let buffer = new ArrayBuffer(4, { maxByteLength: 8 });
        let view = new DataView(buffer, 1);
        let lengths = [view.byteLength];
        buffer.resize(6);
        lengths.push(view.byteLength);
        buffer.resize(1);
        lengths.push(view.byteLength);
        buffer.resize(0);
        let outOfBounds = false;
        try { view.byteLength; } catch (error) {
            outOfBounds = error instanceof TypeError;
        }

        let customBuffer = new ArrayBuffer(3, { maxByteLength: 3 });
        let newTarget = function () {}.bind(null);
        Object.defineProperty(newTarget, "prototype", {
            get: function () {
                customBuffer.resize(2);
                return undefined;
            }
        });
        let custom = Reflect.construct(DataView, [customBuffer, 2], newTarget);

        lengths.join("|") === "3|5|0" && outOfBounds &&
            custom.byteOffset === 2 && custom.byteLength === 0 &&
            Object.getPrototypeOf(custom) === DataView.prototype ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if &actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
