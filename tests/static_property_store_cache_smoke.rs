use velum::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn compiled_static_property_assignment_keeps_access_site_slot() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval("let holder = { slot: 40 }; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;

    let script = vm.compile("holder.slot = holder.slot + 1; holder.slot")?;
    ensure_usize(script.usage().static_property_access_count(), 3)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(41.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn cached_static_property_assignment_falls_back_after_shape_changes() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(
        r"
        let proto = { slot: 10 };
        let holder = { slot: 1 };
        holder.__proto__ = proto;
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;

    let script = vm.compile("holder.slot = 7; holder.slot")?;
    ensure_usize(script.usage().static_property_access_count(), 2)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(7.0))?;
    let value = vm.eval("delete holder.slot")?;
    ensure_value(&value, &Value::Bool(true))?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(7.0))?;
    let value = vm.eval("holder.slot")?;
    ensure_value(&value, &Value::Number(7.0))
}

#[test]
fn cached_static_property_assignment_preserves_non_writable_descriptors() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(
        r"
        let holder = {};
        Object.defineProperty(holder, 'locked', {
            value: 41,
            writable: false,
            enumerable: true,
            configurable: true
        });
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;

    let script = vm.compile("holder.locked = 42; holder.locked")?;
    ensure_usize(script.usage().static_property_access_count(), 2)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(41.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(41.0))
}

#[test]
fn computed_literal_assignment_uses_static_store_operand() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval("let holder = { slot: 40 }; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let script = vm.compile("holder['slot'] = holder['slot'] + 2; holder['slot']")?;
    ensure_usize(script.usage().static_property_access_count(), 3)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
