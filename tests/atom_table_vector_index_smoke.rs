use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_atom_lookup_for_out_of_order_property_names() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let initial_atoms = vm.resource_usage().atom_count;
    let value = vm.eval(
        r"
        let record = { zeta: 1, alpha: 2, middle: 3 };
        record.alpha + record.zeta + record.middle
        ",
    )?;
    ensure_value(&value, &Value::Number(6.0))?;
    let materialized_atoms = vm.resource_usage().atom_count;
    ensure_greater_than(materialized_atoms, initial_atoms, "materialized atoms")?;

    let value = vm.eval("record.middle + record.alpha + record.zeta")?;
    ensure_value(&value, &Value::Number(6.0))?;
    ensure_usize(vm.resource_usage().atom_count, materialized_atoms)?;

    let value = vm.eval("record.absent")?;
    ensure_value(&value, &Value::Undefined)?;
    ensure_usize(vm.resource_usage().atom_count, materialized_atoms)?;

    let value = vm.eval("record.beta = 4; record.beta + record.alpha")?;
    ensure_value(&value, &Value::Number(6.0))?;
    let extended_atoms = vm.resource_usage().atom_count;
    ensure_greater_than(extended_atoms, materialized_atoms, "extended atoms")?;

    let value = vm.eval("record.beta + record.zeta")?;
    ensure_value(&value, &Value::Number(5.0))?;
    ensure_usize(vm.resource_usage().atom_count, extended_atoms)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_greater_than(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual > minimum {
        return Ok(());
    }
    Err(format!("expected {label} greater than {minimum}, got {actual}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
