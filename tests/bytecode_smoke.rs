use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const BYTECODE_LOOP_SOURCE: &str = r"
var camera0 = 1;
var camera1 = 2;
var camera2 = 3;
var camera3 = 4;
var total = 0;
var index = 0;
while (index < 32) {
    var slot = index & 3;
    total = total + camera0 + camera1 + camera2 + camera3 + slot;
    index = index + 1;
}
total;
";

const BYTECODE_PROPERTY_SOURCE: &str = r"
var values = [1, 2, 3, 4];
var holder = { offset: 10 };
holder.total = values[0] + values[1] + holder.offset;
holder.total;
";

#[test]
fn compiled_script_exposes_bytecode_instruction_count() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile("let value = 1 + 2 + 3; value")?;

    ensure_positive(
        script.usage().bytecode_instruction_count(),
        "bytecode instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(6.0))
}

#[test]
fn bytecode_executes_repeated_binding_loop_without_atom_growth() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_LOOP_SOURCE)?;
    ensure_positive(
        script.usage().bytecode_instruction_count(),
        "bytecode instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(368.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(368.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_executes_property_array_and_object_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_PROPERTY_SOURCE)?;
    ensure_positive(
        script.usage().bytecode_instruction_count(),
        "bytecode instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(13.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(13.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual != expected {
        return Err(format!("expected {expected:?}, got {actual:?}").into());
    }
    Ok(())
}

fn ensure_positive(value: usize, label: &str) -> TestResult {
    if value == 0 {
        return Err(format!("expected positive {label}, got {value}").into());
    }
    Ok(())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual != expected {
        return Err(format!("expected {expected}, got {actual}").into());
    }
    Ok(())
}
