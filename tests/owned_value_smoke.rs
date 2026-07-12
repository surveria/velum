use rs_quickjs::{Engine, Error, JsBigInt, OwnedValue, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn evaluates_every_portable_primitive_kind() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    for (source, expected) in [
        ("undefined", OwnedValue::Undefined),
        ("null", OwnedValue::Null),
        ("true", OwnedValue::Bool(true)),
        ("42", OwnedValue::Number(42.0)),
        ("42n", OwnedValue::BigInt(JsBigInt::from_u64(42))),
        (r#""camera""#, OwnedValue::String("camera".to_owned())),
    ] {
        let actual = vm.eval_owned(source)?;
        ensure_owned(&actual, &expected)?;
    }
    Ok(())
}

#[test]
fn evaluates_compiled_source_into_an_owned_value() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(r#""compiled-camera""#)?;

    let actual = vm.eval_compiled_owned(&script)?;
    ensure_owned(&actual, &OwnedValue::String("compiled-camera".to_owned()))
}

#[test]
fn moves_an_owned_heap_string_between_independent_vms() -> TestResult {
    let engine = Engine::new();
    let mut first_vm = engine.create_vm();
    let owned = first_vm.eval_owned(r#""portable-camera""#)?;
    drop(first_vm);

    let mut second_vm = engine.create_vm();
    second_vm.register_host_function_typed("hostPortable", move |_call| Ok(owned.clone()))?;
    let actual = second_vm.eval_owned("hostPortable()")?;
    ensure_owned(&actual, &OwnedValue::String("portable-camera".to_owned()))
}

#[test]
fn copies_callback_local_heap_strings_into_owned_values() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_host_function_typed("hostOwn", |call| {
        call.required_value(0, "value")?.to_owned_value()
    })?;

    let actual = vm.eval_owned(r#"hostOwn("callback-camera")"#)?;
    ensure_owned(&actual, &OwnedValue::String("callback-camera".to_owned()))
}

#[test]
fn rejects_vm_local_symbol_object_and_function_values() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    for source in [
        "Symbol('camera')",
        "({ camera: 1 })",
        "(function camera() {})",
    ] {
        let Err(error) = vm.eval_owned(source) else {
            return Err(format!("expected owned conversion to reject {source}").into());
        };
        ensure_local_value_error(&error)?;
    }
    Ok(())
}

#[test]
fn owned_values_convert_back_to_plain_runtime_primitives() -> TestResult {
    for (owned, expected) in [
        (OwnedValue::Undefined, Value::Undefined),
        (OwnedValue::Null, Value::Null),
        (OwnedValue::Bool(true), Value::Bool(true)),
        (OwnedValue::Number(42.0), Value::Number(42.0)),
        (
            OwnedValue::BigInt(JsBigInt::from_u64(42)),
            Value::BigInt(JsBigInt::from_u64(42)),
        ),
        (
            OwnedValue::String("camera".to_owned()),
            Value::from("camera"),
        ),
    ] {
        let actual = Value::from(owned);
        if actual != expected {
            return Err(format!("expected {expected:?}, got {actual:?}").into());
        }
    }
    Ok(())
}

fn ensure_owned(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_local_value_error(error: &Error) -> TestResult {
    if matches!(error, Error::Runtime { .. })
        && error
            .to_string()
            .contains("VM-local value cannot be converted to OwnedValue")
    {
        return Ok(());
    }
    Err(format!("expected a VM-local conversion error, got {error:?}").into())
}
