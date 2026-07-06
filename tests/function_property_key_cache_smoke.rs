use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn dynamic_function_property_paths_reuse_known_keys() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r"
        let local = function local() {};
        let native = Math.abs;
        let localName = 'dynamicLocalSlot';
        let nativeName = 'dynamicNativeSlot';
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;
    let declared_atoms = vm.resource_usage().atom_count;

    let value = vm.context().eval("local[localName]; native[nativeName]")?;
    ensure_value(&value, &Value::Undefined)?;
    ensure_usize(vm.resource_usage().atom_count, declared_atoms)?;

    let value = vm.context().eval(
        r"
        local[localName] = 1;
        native[nativeName] = 2;
        local[localName] + native[nativeName]
        ",
    )?;
    ensure_value(&value, &Value::Number(3.0))?;
    let inserted_atoms = vm.resource_usage().atom_count;
    ensure_greater_than(
        inserted_atoms,
        declared_atoms,
        "dynamic function property atoms",
    )?;

    let value = vm.context().eval(
        r"
        local[localName] += 3;
        native[nativeName]++;
        local[localName] + native[nativeName]
        ",
    )?;
    ensure_value(&value, &Value::Number(7.0))?;
    ensure_usize(vm.resource_usage().atom_count, inserted_atoms)?;

    let value = vm
        .context()
        .eval("delete local[localName] && delete native[nativeName]")?;
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
