use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn shares_bytes_across_views_and_grows_without_shrinking() -> TestResult {
    ensure_eval(
        r"
        let buffer = new SharedArrayBuffer(8, { maxByteLength: 16 });
        let bytes = new Uint8Array(buffer);
        let data = new DataView(buffer);
        bytes[0] = 17;
        data.setUint8(1, 23);
        buffer.grow(12);
        let grown = new Uint8Array(buffer);
        let copied = buffer.slice(0, 2);
        let failures = 0;
        try { buffer.grow(4); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { ArrayBuffer.prototype.slice.call(buffer); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        buffer.byteLength === 12 && buffer.maxByteLength === 16 && buffer.growable &&
            bytes.length === 12 && grown.length === 12 && grown[0] === 17 && grown[1] === 23 &&
            copied instanceof SharedArrayBuffer && copied.byteLength === 2 &&
            new Uint8Array(copied).join(',') === '17,23' && failures === 2 ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn performs_number_and_bigint_atomic_updates() -> TestResult {
    ensure_eval(
        r"
        let buffer = new SharedArrayBuffer(32);
        let ints = new Int32Array(buffer, 0, 2);
        let big = new BigUint64Array(buffer, 16, 2);
        let values = [];
        values.push(Atomics.store(ints, 0, 5));
        values.push(Atomics.add(ints, 0, 3));
        values.push(Atomics.sub(ints, 0, 2));
        values.push(Atomics.and(ints, 0, 3));
        values.push(Atomics.or(ints, 0, 8));
        values.push(Atomics.xor(ints, 0, 1));
        values.push(Atomics.exchange(ints, 0, -2));
        values.push(Atomics.compareExchange(ints, 0, -2, 9));
        values.push(Atomics.load(ints, 0));
        Atomics.store(big, 0, 18446744073709551615n);
        let oldBig = Atomics.add(big, 0, 2n);
        let nowBig = Atomics.load(big, 0);
        values.join(',') === '5,5,8,6,2,10,11,-2,9' &&
            oldBig === 18446744073709551615n && nowBig === 1n ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn exposes_metadata_and_rejects_non_shared_or_float_views() -> TestResult {
    ensure_eval(
        r"
        let failures = 0;
        try { SharedArrayBuffer(8); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        let local = new Int32Array(new ArrayBuffer(4));
        Atomics.store(local, 0, 7);
        try { Atomics.load(new Float32Array(new SharedArrayBuffer(4)), 0); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        SharedArrayBuffer.name === 'SharedArrayBuffer' && SharedArrayBuffer.length === 1 &&
            SharedArrayBuffer.prototype[Symbol.toStringTag] === 'SharedArrayBuffer' &&
            Atomics[Symbol.toStringTag] === 'Atomics' && Atomics.add.length === 3 &&
            Atomics.compareExchange.length === 4 && Atomics.notify.length === 3 &&
            Atomics.wait.length === 4 && Atomics.pause.length === 0 &&
            Atomics.isLockFree(1) && Atomics.isLockFree(8) && !Atomics.isLockFree(3) &&
            Atomics.load(local, 0) === 7 && failures === 2 ? 42 : 0
        ",
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
