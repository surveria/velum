use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

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

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if &actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
