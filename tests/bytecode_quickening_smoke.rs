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

#[test]
fn bytecode_quickens_static_array_index_reads_and_writes_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var values = [10, 20];
        var first = values[0];
        values[1] = first + 2;
        var missing = values[3];
        values[3] = 99;

        var plain = {};
        plain[0] = 5;
        plain[0] = plain[0] + 1;

        var text = "go";
        first === 10 &&
            values[1] === 12 &&
            missing === undefined &&
            values.length === 4 &&
            values[3] === 99 &&
            plain[0] === 6 &&
            text[0] === "g" ? 42 : 0
        "#,
    )?;
    ensure_at_least(
        script.usage().bytecode_property_operand_count(),
        8,
        "bytecode property operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_quickens_dynamic_array_index_reads_and_writes_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var values = [4, 5, 6];
        var index = 1;
        var first = values[index];
        values[index + 1] = first + 10;

        var key = "0";
        var fromStringKey = values[key];

        var far = 100000;
        values[far] = 7;

        var plain = {};
        plain[index] = 8;

        var text = "hi";
        var zero = 0;

        first === 5 &&
            values[2] === 15 &&
            fromStringKey === 4 &&
            values[far] === 7 &&
            plain[1] === 8 &&
            text[zero] === "h" ? 42 : 0
        "#,
    )?;
    ensure_at_least(
        script.usage().bytecode_property_operand_count(),
        9,
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
