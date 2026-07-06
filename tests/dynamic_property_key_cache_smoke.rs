use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn dynamic_missing_property_paths_do_not_intern_names() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r"
        let holder = { known: 1 };
        let missingName = 'missingDynamicSlot';
        holder[missingName]
        ",
    )?;
    ensure_value(&value, &Value::Undefined)?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.context().eval("delete holder[missingName]")?;
    ensure_value(&value, &Value::Bool(true))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn dynamic_property_operations_reuse_known_property_keys() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r"
        let holder = {};
        let propertyName = 'dynamicCameraSlot';
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;
    let initial_atoms = vm.resource_usage().atom_count;

    let value = vm
        .context()
        .eval("holder[propertyName] = 1; holder[propertyName]")?;
    ensure_value(&value, &Value::Number(1.0))?;
    let inserted_atoms = vm.resource_usage().atom_count;
    ensure_greater_than(inserted_atoms, initial_atoms, "dynamic property atom")?;

    let value = vm.context().eval(
        r"
        holder[propertyName] += 2;
        holder[propertyName]++;
        holder[propertyName]
        ",
    )?;
    ensure_value(&value, &Value::Number(4.0))?;
    ensure_usize(vm.resource_usage().atom_count, inserted_atoms)?;

    let value = vm.context().eval("delete holder[propertyName]")?;
    ensure_value(&value, &Value::Bool(true))?;
    ensure_usize(vm.resource_usage().atom_count, inserted_atoms)
}

#[test]
fn compiled_dynamic_computed_operations_have_access_site_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var holder = { slot: 1 };
        var key = "slot";
        holder[key] = holder[key] + 1;
        holder[key] += 2;
        holder[key]++;
        holder[key];
        "#,
    )?;

    ensure_usize(script.usage().static_property_access_count(), 5)?;
    ensure_usize(script.usage().bytecode_property_operand_count(), 5)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(5.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(5.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn compiled_dynamic_computed_calls_preserve_this_binding() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var receiver = {
            value: 41,
            read: function () {
                return this.value + 1;
            }
        };
        var key = "read";
        receiver[key]();
        "#,
    )?;

    ensure_usize(script.usage().static_property_access_count(), 2)?;
    ensure_usize(script.usage().bytecode_property_operand_count(), 2)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn compiled_dynamic_access_site_uses_current_property_key() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var holder = { alpha: 1, beta: 2 };
        var read = function (key) {
            return holder[key];
        };
        read("alpha") * 10 + read("beta");
        "#,
    )?;

    ensure_usize(script.usage().static_property_access_count(), 1)?;
    ensure_usize(script.usage().bytecode_property_operand_count(), 1)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(12.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(12.0))
}

#[test]
fn compiled_dynamic_array_indices_keep_array_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var values = [0];
        var key = 0;
        values[key] = 42;
        values[key];
        ",
    )?;

    ensure_usize(script.usage().static_property_access_count(), 2)?;
    ensure_usize(script.usage().bytecode_property_operand_count(), 2)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn compiled_dynamic_delete_and_in_have_bytecode_property_operands() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var holder = { slot: 1 };
        var key = "slot";
        var before = key in holder;
        var removed = delete holder[key];
        var after = key in holder;
        before && removed && !after ? 17 : 0;
        "#,
    )?;

    ensure_usize(script.usage().static_property_access_count(), 3)?;
    ensure_usize(script.usage().bytecode_property_operand_count(), 3)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(17.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(17.0))
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

fn ensure_greater_than(actual: usize, baseline: usize, label: &str) -> TestResult {
    if actual > baseline {
        return Ok(());
    }
    Err(format!("{label} should be greater than {baseline}, got {actual}").into())
}
