use rs_quickjs::{HostOperation, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const DETACH_NAME: &str = "hostDetachArrayBuffer";

#[test]
fn typed_array_at_uses_internal_length() -> TestResult {
    ensure_eval(
        r#"
        let array = Object.defineProperty(new Uint8Array([1, 2, 3]), "length", {
            get() { throw new Error("length accessor called"); }
        });
        array.at(1) === 2 && array.at(-1) === 3 ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn typed_array_at_snapshots_length_before_index_coercion() -> TestResult {
    ensure_eval(
        r"
        let buffer = new ArrayBuffer(4, { maxByteLength: 8 });
        let array = new Uint8Array(buffer);
        array[3] = 7;
        let index = {
            valueOf() {
                buffer.resize(2);
                return -1;
            }
        };
        array.at(index) === undefined ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn typed_array_integrity_accounts_for_indexed_elements() -> TestResult {
    ensure_eval(
        r"
        let sealed = new Int32Array(2);
        let sealThrew = false;
        try { Object.seal(sealed); } catch (error) {
            sealThrew = error instanceof TypeError;
        }
        let frozen = new Int32Array(1);
        let freezeThrew = false;
        try { Object.freeze(frozen); } catch (error) {
            freezeThrew = error instanceof TypeError;
        }
        let empty = new Int32Array(0);
        Object.preventExtensions(empty);
        sealThrew && freezeThrew && !Object.isExtensible(sealed) &&
            !Object.isSealed(sealed) && !Object.isFrozen(frozen) &&
            Object.isSealed(empty) && Object.isFrozen(empty) ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn variable_length_views_control_prevent_extensions_and_integrity() -> TestResult {
    ensure_eval(
        r"
        let rab = new ArrayBuffer(8, { maxByteLength: 16 });
        let rabFixed = new Int32Array(rab, 0, 0);
        let rabTracking = new Int32Array(rab);
        let gsab = new SharedArrayBuffer(8, { maxByteLength: 16 });
        let gsabFixed = new Int32Array(gsab, 0, 0);
        let gsabTracking = new Int32Array(gsab);

        let rabPrevented = Reflect.preventExtensions(rabFixed);
        let trackingPrevented = Reflect.preventExtensions(rabTracking);
        let gsabFixedPrevented = Reflect.preventExtensions(gsabFixed);
        let gsabTrackingPrevented = Reflect.preventExtensions(gsabTracking);
        let rabSealThrew = false;
        try { Object.seal(new Int32Array(rab, 0, 0)); } catch (error) {
            rabSealThrew = error instanceof TypeError;
        }
        let gsabTrackingSealThrew = false;
        try { Object.seal(new Int32Array(gsab)); } catch (error) {
            gsabTrackingSealThrew = error instanceof TypeError;
        }
        let gsabEmpty = new Int32Array(gsab, 0, 0);
        Object.seal(gsabEmpty);

        !rabPrevented && !trackingPrevented && gsabFixedPrevented &&
            !gsabTrackingPrevented && rabSealThrew && gsabTrackingSealThrew &&
            Object.isSealed(gsabEmpty) ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn observes_coercion_before_detached_backing_store_checks() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(DETACH_NAME, HostOperation::DetachArrayBuffer)?;
    let actual = context.eval(
        r"
        let constructorBuffer = new ArrayBuffer(0);
        hostDetachArrayBuffer(constructorBuffer);
        let alignmentWon = false;
        try { new Int32Array(constructorBuffer, 1, 0); } catch (error) {
            alignmentWon = error instanceof RangeError;
        }

        let targetBuffer = new ArrayBuffer(4);
        let target = new Int32Array(targetBuffer);
        hostDetachArrayBuffer(targetBuffer);
        let marker = {};
        let offsetWon = false;
        try {
            target.set(null, { valueOf() { throw marker; } });
        } catch (error) {
            offsetWon = error === marker;
        }
        alignmentWon && offsetWon ? 42 : 0
        ",
    )?;
    if actual == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected detached backing-store ordering, got {actual:?}").into())
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
