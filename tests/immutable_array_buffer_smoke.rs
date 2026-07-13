use rs_quickjs::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn creates_readable_immutable_buffers_and_rejects_mutation() -> TestResult {
    ensure_eval(
        r#"
        let source = new ArrayBuffer(4);
        new Uint8Array(source).set([1, 2, 3, 4]);
        let immutable = source.transferToImmutable();
        let view = new Uint8Array(immutable);
        let dataView = new DataView(immutable);
        let firstByte = dataView.getUint8(0);
        let dataViewRejected = false;
        let typedArrayRejected = false;
        try { dataView.setUint8(0, 9); } catch (error) {
            dataViewRejected = error instanceof TypeError;
        }
        try { view[0] = 9; } catch (error) {
            typedArrayRejected = error instanceof TypeError;
        }
        source.detached && immutable.immutable && !immutable.resizable &&
            firstByte === 1 && view.join(",") === "1,2,3,4" &&
            dataViewRejected && typedArrayRejected ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn slices_to_independent_immutable_storage() -> TestResult {
    ensure_eval(
        r#"
        let source = new ArrayBuffer(4, { maxByteLength: 8 });
        new Uint8Array(source).set([1, 2, 3, 4]);
        let immutable = source.sliceToImmutable(1, 3);
        source.resize(8);
        new Uint8Array(source)[1] = 9;
        immutable.immutable && immutable.byteLength === 2 &&
            new Uint8Array(immutable).join(",") === "2,3" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn preserves_same_buffer_slice_species_aliasing() -> TestResult {
    ensure_eval(
        r#"
        let source = new Uint8Array([10, 20, 30, 40, 50, 60]);
        let immutable = source.buffer.transferToImmutable();
        let view = new Uint8Array(immutable);
        view.constructor = {
            [Symbol.species]: function() {
                return new Uint8Array(view.buffer, 2);
            }
        };
        view.slice(1, 4).join(",") === "20,20,20,60" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if actual == *expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
