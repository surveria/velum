use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bytecode_quickens_numeric_add_and_array_length_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var values = [1, 2, 3];
        var plain = { length: 9 };
        var numeric = values.length + 4 + 5;
        var text = "len=" + values.length;
        var fallbackTotal = plain.length + "go".length + Math.max.length;
        numeric === 12 &&
            text === "len=3" &&
            fallbackTotal === 13 ? 42 : 0
        "#,
    )?;
    let usage = script.usage();

    ensure_at_least(
        usage.bytecode_numeric_instruction_count(),
        5,
        "bytecode numeric instructions",
    )?;
    ensure_at_least(
        usage.bytecode_property_operand_count(),
        5,
        "bytecode property operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(value: usize, minimum: usize, label: &str) -> TestResult {
    if value >= minimum {
        return Ok(());
    }
    Err(format!("expected {label} >= {minimum}, got {value}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
