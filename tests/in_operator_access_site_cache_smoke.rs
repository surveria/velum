use velum::{Engine, Value};

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

#[test]
fn compiled_in_operator_hot_paths_run_linear_segments_without_interning_missing_keys() -> TestResult
{
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var total = 0;
        var object = { a: 1 };
        var values = [1, 2, 3, 4];
        var inherited = [];
        inherited.length = 4;
        Object.setPrototypeOf(inherited, { 2: 1 });

        for (var index = 0; index < 16; index = index + 1) {
            if ("a" in object) {
                total += object.a;
            }
            if ((index & 3) in values) {
                total += 1;
            }
            if ((index & 3) in inherited) {
                total += 2;
            }
            if ("missing" in object) {
                total += 64;
            }
        }

        total
        "#,
    )?;

    let segment_runs = vm.resource_usage().bytecode_linear_segment_runs;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(40.0))?;
    let segment_delta = vm
        .resource_usage()
        .bytecode_linear_segment_runs
        .checked_sub(segment_runs)
        .ok_or("bytecode linear segment counter moved backwards")?;
    ensure_at_least(segment_delta, 16, "bytecode linear segment runs")?;

    let atom_count = vm.resource_usage().atom_count;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(40.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
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

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }
    Err(format!("expected {label} >= {expected}, got {actual}").into())
}
