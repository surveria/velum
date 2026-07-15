use velum::{Engine, Error, Runtime, RuntimeLimits, Value, VmStorageKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_array_buffer_metadata_and_view_detection() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const buffer = new ArrayBuffer(8);
        const typed = new Uint8Array(buffer);
        const view = new DataView(buffer);
        const descriptor = Object.getOwnPropertyDescriptor(
            ArrayBuffer.prototype,
            "byteLength"
        );

        ArrayBuffer.isView(buffer) === false &&
            ArrayBuffer.isView(typed) === true &&
            ArrayBuffer.isView(view) === true &&
            ArrayBuffer.isView({}) === false &&
            buffer.byteLength === 8 &&
            buffer.maxByteLength === 8 &&
            buffer.resizable === false &&
            buffer.detached === false &&
            descriptor.get.name === "get byteLength" &&
            Object.prototype.toString.call(buffer) === "[object ArrayBuffer]"
            ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn resizes_shared_backing_storage_and_zero_fills_growth() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        const buffer = new ArrayBuffer(4, { maxByteLength: 12 });
        const fixed = new Uint8Array(buffer, 0, 4);
        const tracking = new Uint8Array(buffer);
        tracking[0] = 7;
        tracking[3] = 9;
        buffer.resize(8);
        const grew = buffer.byteLength === 8 &&
            buffer.maxByteLength === 12 &&
            buffer.resizable === true &&
            fixed.length === 4 && tracking.length === 8 &&
            tracking[0] === 7 && tracking[3] === 9 && tracking[7] === 0;
        buffer.resize(2);
        const shrank = buffer.byteLength === 2 && fixed.length === 0 &&
            fixed[0] === undefined && tracking.length === 2 && tracking[0] === 7;
        grew && shrank ? 42 : 0
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn slices_and_transfers_array_buffers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        const source = new ArrayBuffer(6, { maxByteLength: 10 });
        const bytes = new Uint8Array(source);
        const dataView = new DataView(source);
        bytes[1] = 11;
        bytes[2] = 22;
        bytes[3] = 33;
        const sliced = source.slice(1, 4);
        const slicedBytes = new Uint8Array(sliced);
        const transferred = source.transfer(8);
        const transferredBytes = new Uint8Array(transferred);
        let dataViewDetached = false;
        try {
            dataView.byteLength;
        } catch (error) {
            dataViewDetached = error instanceof TypeError;
        }

        sliced.byteLength === 3 &&
            slicedBytes[0] === 11 && slicedBytes[1] === 22 && slicedBytes[2] === 33 &&
            source.detached === true && source.byteLength === 0 &&
            bytes.length === 0 && bytes.byteLength === 0 && bytes.byteOffset === 0 &&
            bytes[0] === undefined && dataViewDetached === true &&
            transferred.byteLength === 8 && transferred.maxByteLength === 10 &&
            transferred.resizable === true && transferredBytes[1] === 11 &&
            transferredBytes[2] === 22 && transferredBytes[6] === 0
            ? 42 : 0
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn reconciles_live_buffer_payload_after_resize_and_transfer() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval("var buffer = new ArrayBuffer(4, { maxByteLength: 12 });")?;
    ensure_usize(
        vm.storage_snapshot()?
            .payload_bytes(VmStorageKind::ByteBuffer),
        4,
        "initial byte payload",
    )?;
    vm.eval("buffer.resize(10);")?;
    ensure_usize(
        vm.storage_snapshot()?
            .payload_bytes(VmStorageKind::ByteBuffer),
        10,
        "resized byte payload",
    )?;
    vm.eval("buffer = buffer.transferToFixedLength(3);")?;
    ensure_usize(
        vm.storage_snapshot()?
            .payload_bytes(VmStorageKind::ByteBuffer),
        3,
        "transferred byte payload",
    )
}

#[test]
fn preserves_reduce_iteration_contracts_across_resize() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        const genericBuffer = new ArrayBuffer(4, { maxByteLength: 4 });
        const generic = new Uint8Array(genericBuffer);
        const genericIndices = [];
        Array.prototype.reduce.call(generic, function(previous, next, index) {
            if (index === 0) genericBuffer.resize(2);
            genericIndices.push(index);
            return next;
        }, 0);

        const typedBuffer = new ArrayBuffer(4, { maxByteLength: 4 });
        const typed = new Uint8Array(typedBuffer);
        const typedValues = [];
        typed.reduce(function(previous, next, index) {
            if (index === 0) typedBuffer.resize(2);
            typedValues.push(next);
            return next;
        }, 0);

        genericIndices.length === 2 && genericIndices[0] === 0 &&
            genericIndices[1] === 1 && typedValues.length === 4 &&
            typedValues[0] === 0 && typedValues[1] === 0 &&
            typedValues[2] === undefined && typedValues[3] === undefined
            ? 42 : 0
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn separates_byte_buffer_and_object_property_limits() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_object_properties: 128,
        max_byte_buffer_len: 512,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let value = context.eval("new Uint8Array(256).byteLength")?;
    ensure_value(&value, &Value::Number(256.0))?;

    let runtime = Runtime::with_limits(RuntimeLimits {
        max_byte_buffer_len: 4,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let Err(error) = context.eval("new ArrayBuffer(5)") else {
        return Err("expected the byte buffer limit to reject ArrayBuffer allocation".into());
    };
    if matches!(error, Error::ResourceLimit { .. })
        && error
            .to_string()
            .contains("typed array byte length exceeded 4")
    {
        let value = context.eval(
            "try { new Uint8Array(new ArrayBuffer(0), 0, 1000000); 0; } \
             catch (error) { error instanceof RangeError ? 42 : 0; }",
        )?;
        return ensure_value(&value, &Value::Number(42.0));
    }
    Err(format!("expected byte buffer resource limit, got {error:?}").into())
}

#[test]
fn resolves_new_target_prototypes_before_backing_store_limits() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        function ExpectedError() {}
        function throwingNewTarget(trace) {
            let target = function () {}.bind(null);
            Object.defineProperty(target, "prototype", {
                get() {
                    trace.push("prototype");
                    throw new ExpectedError();
                }
            });
            return target;
        }
        function hugeLength(trace) {
            return {
                valueOf() {
                    trace.push("length");
                    return 7 * 1125899906842624;
                }
            };
        }

        let arrayTrace = [];
        let arrayThrew = false;
        try {
            Reflect.construct(
                ArrayBuffer,
                [hugeLength(arrayTrace)],
                throwingNewTarget(arrayTrace)
            );
        } catch (error) {
            arrayThrew = error instanceof ExpectedError;
        }
        let sharedTrace = [];
        let sharedThrew = false;
        try {
            Reflect.construct(
                SharedArrayBuffer,
                [hugeLength(sharedTrace)],
                throwingNewTarget(sharedTrace)
            );
        } catch (error) {
            sharedThrew = error instanceof ExpectedError;
        }
        arrayThrew && sharedThrew && arrayTrace.join() === "length,prototype" &&
            sharedTrace.join() === "length,prototype" ? 42 : 0
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

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("{label}: expected {expected}, got {actual}").into())
}
