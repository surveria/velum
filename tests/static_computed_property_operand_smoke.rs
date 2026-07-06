use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn string_and_number_computed_literals_use_static_property_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        let holder = { slot: 40, 0: 2 };
        holder["slot"] + holder[0]
        "#,
    )?;

    ensure_usize(script.usage().static_property_access_count(), 2)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn static_computed_literal_reads_do_not_intern_missing_property_names() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.context().eval("let holder = { present: 41 }; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;

    let atom_count = vm.resource_usage().atom_count;
    let script = vm.compile(
        r#"
        holder["present"] + (holder["missingStaticComputedSlot"] === undefined ? 1 : 0)
        "#,
    )?;
    ensure_usize(script.usage().static_property_access_count(), 2)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn static_computed_literal_assignments_reuse_existing_property_atoms() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.context().eval("let holder = { present: 39 }; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let script = vm.compile(
        r#"
        holder["present"] += 2;
        holder["present"]++;
        holder["present"]
        "#,
    )?;
    ensure_usize(script.usage().static_property_access_count(), 3)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn static_computed_literal_calls_preserve_this_binding() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.context().eval(
        r#"
        let receiver = {
            value: 42,
            read() { return this.value; }
        };
        receiver["read"]()
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
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
