use velum::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn creates_one_final_shape_for_wide_integrity_transitions() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm
        .context()
        .eval("Object.seal({ warmup: 1 }); Object.freeze({ warmup: 1 }); true")?;
    ensure_value(&value, &Value::Bool(true))?;

    let value = vm.context().eval(
        r#"
        let sealed = {};
        for (let index = 0; index < 64; index = index + 1) {
            sealed["field" + index] = index;
        }
        sealed.field63
        "#,
    )?;
    ensure_value(&value, &Value::Number(63.0))?;
    let open_shapes = vm.resource_usage().shape_count;

    let value = vm
        .context()
        .eval("Object.seal(sealed); Object.isSealed(sealed)")?;
    ensure_value(&value, &Value::Bool(true))?;
    let sealed_shapes = open_shapes
        .checked_add(1)
        .ok_or("sealed shape count overflowed")?;
    ensure_usize(vm.resource_usage().shape_count, sealed_shapes)?;
    vm.storage_snapshot()?;

    let value = vm.context().eval(
        r#"
        let repeatedSeal = {};
        for (let index = 0; index < 64; index = index + 1) {
            repeatedSeal["field" + index] = index + 1;
        }
        Object.seal(repeatedSeal);
        repeatedSeal.field63
        "#,
    )?;
    ensure_value(&value, &Value::Number(64.0))?;
    ensure_usize(vm.resource_usage().shape_count, sealed_shapes)?;

    let value = vm.context().eval(
        r#"
        let frozen = {};
        for (let index = 0; index < 64; index = index + 1) {
            frozen["field" + index] = index;
        }
        Object.freeze(frozen);
        Object.isFrozen(frozen)
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))?;
    let frozen_shapes = sealed_shapes
        .checked_add(1)
        .ok_or("frozen shape count overflowed")?;
    ensure_usize(vm.resource_usage().shape_count, frozen_shapes)?;

    let value = vm.context().eval(
        r#"
        let repeatedFreeze = {};
        for (let index = 0; index < 64; index = index + 1) {
            repeatedFreeze["field" + index] = index + 2;
        }
        Object.freeze(repeatedFreeze);
        repeatedFreeze.field63
        "#,
    )?;
    ensure_value(&value, &Value::Number(65.0))?;
    ensure_usize(vm.resource_usage().shape_count, frozen_shapes)?;
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn value_writes_do_not_create_shape_transitions() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r#"
        let wide = {};
        for (let index = 0; index < 64; index = index + 1) {
            wide["slot" + index] = index;
        }
        wide.slot63
        "#,
    )?;
    ensure_value(&value, &Value::Number(63.0))?;
    let shapes = vm.resource_usage().shape_count;

    let value = vm.context().eval(
        r#"
        for (let round = 0; round < 8; round = round + 1) {
            for (let index = 0; index < 64; index = index + 1) {
                wide["slot" + index] = round + index;
            }
        }
        wide.slot63
        "#,
    )?;
    ensure_value(&value, &Value::Number(70.0))?;
    ensure_usize(vm.resource_usage().shape_count, shapes)?;
    vm.storage_snapshot()?;
    Ok(())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
