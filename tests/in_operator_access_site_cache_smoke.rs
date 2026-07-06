use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn compiled_in_operator_has_occurrence_slots_without_interning_missing_keys() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().eval("var object = { present: 1 };")?;
    let atom_count = vm.resource_usage().atom_count;
    let script = vm.compile(r#""present" in object && !("missing" in object)"#)?;

    ensure_usize(script.usage().static_property_access_count(), 2)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Bool(true))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Bool(true))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn cached_in_operator_presence_follows_shape_and_prototype_changes() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().eval(
        r"
        var proto = { slot: 1 };
        var child = { __proto__: proto };
        ",
    )?;
    let inherited = vm.compile(r#""slot" in child"#)?;
    let own = vm.compile(r#""own" in child"#)?;

    ensure_usize(inherited.usage().static_property_access_count(), 1)?;
    ensure_usize(own.usage().static_property_access_count(), 1)?;

    let value = vm.eval_compiled(&inherited)?;
    ensure_value(&value, &Value::Bool(true))?;

    let value = vm.context().eval("delete proto.slot; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;
    let value = vm.eval_compiled(&inherited)?;
    ensure_value(&value, &Value::Bool(false))?;

    let value = vm.context().eval("child.slot = 2; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;
    let value = vm.eval_compiled(&inherited)?;
    ensure_value(&value, &Value::Bool(true))?;

    let value = vm.eval_compiled(&own)?;
    ensure_value(&value, &Value::Bool(false))?;
    let value = vm.context().eval("child.own = 3; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;
    let value = vm.eval_compiled(&own)?;
    ensure_value(&value, &Value::Bool(true))
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
    Err(format!("expected usize {expected}, got {actual}").into())
}
