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
