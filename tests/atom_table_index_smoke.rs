use std::fmt::Write as _;

use velum::{Engine, Value, VmStorageKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const DISTINCT_ATOM_COUNT: usize = 2_048;

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

#[test]
fn interns_many_runtime_atoms_once_with_exact_payload_accounting() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval("var atomWarmup = {}; atomWarmup.warmupField = 1;")?;
    let before = vm.storage_snapshot()?;

    let mut source = String::new();
    source
        .try_reserve(DISTINCT_ATOM_COUNT.saturating_mul(48))
        .map_err(|error| format!("atom test source allocation failed: {error}"))?;
    source.push_str("var atomRecord = {};\n");
    let mut expected_payload_bytes = "atomRecord".len();
    for index in 0..DISTINCT_ATOM_COUNT {
        writeln!(source, "atomRecord.field_{index} = {index};")?;
        expected_payload_bytes = expected_payload_bytes
            .checked_add(format!("field_{index}").len())
            .ok_or("atom payload expectation overflowed")?;
    }
    let last_index = DISTINCT_ATOM_COUNT
        .checked_sub(1)
        .ok_or("distinct atom count is empty")?;
    write!(source, "atomRecord.field_{last_index}")?;

    let script = vm.compile(&source)?;
    let expected_value = f64::from(u32::try_from(last_index)?);
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(expected_value))?;
    let after_first = vm.storage_snapshot()?;
    let expected_count = before
        .count(VmStorageKind::Atom)
        .checked_add(DISTINCT_ATOM_COUNT)
        .and_then(|count| count.checked_add(1))
        .ok_or("atom count expectation overflowed")?;
    ensure_usize(after_first.count(VmStorageKind::Atom), expected_count)?;
    let expected_bytes = before
        .payload_bytes(VmStorageKind::Atom)
        .checked_add(expected_payload_bytes)
        .ok_or("atom payload byte expectation overflowed")?;
    ensure_usize(
        after_first.payload_bytes(VmStorageKind::Atom),
        expected_bytes,
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(expected_value))?;
    let after_second = vm.storage_snapshot()?;
    ensure_usize(
        after_second.count(VmStorageKind::Atom),
        after_first.count(VmStorageKind::Atom),
    )?;
    ensure_usize(
        after_second.payload_bytes(VmStorageKind::Atom),
        after_first.payload_bytes(VmStorageKind::Atom),
    )
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
