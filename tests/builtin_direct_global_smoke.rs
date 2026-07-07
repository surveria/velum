use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn compiled_builtin_calls_use_guarded_direct_values() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        {
          let total = 0;
          total += Math.abs(-3);
          total += Number("4");
          total += Boolean("x") ? 5 : 0;
          total += String(67).length;
          total += Array(2).length;
          total += Object() ? 8 : 0;
          total;
        }
        "#,
    )?;

    let first = vm.eval_compiled(&script)?;
    ensure_value(&first, &Value::Number(24.0))?;
    let usage = vm.resource_usage();

    let second = vm.eval_compiled(&script)?;
    ensure_value(&second, &Value::Number(24.0))?;
    ensure_usize(vm.resource_usage().atom_count, usage.atom_count)?;
    ensure_usize(vm.resource_usage().global_bindings, usage.global_bindings)
}

#[test]
fn direct_builtin_calls_preserve_shadowing_and_mutation() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let shadowed = vm.eval(
        r"
        {
          let Math = { abs(value) { return value + 100; } };
          Math.abs(1);
        }
        ",
    )?;
    ensure_value(&shadowed, &Value::Number(101.0))?;

    let shadowed_call = vm.eval(
        r"
        {
          let Boolean = function(value) { return !value; };
          Boolean(1) ? 0 : 42;
        }
        ",
    )?;
    ensure_value(&shadowed_call, &Value::Number(42.0))?;

    let mutated = vm.eval(
        r"
        Math.abs = function(value) { return value + 200; };
        Math.abs(3);
        ",
    )?;
    ensure_value(&mutated, &Value::Number(203.0))?;

    let typed = vm.eval(
        r#"
        typeof Math === "object" &&
          typeof JSON === "object" &&
          typeof Boolean === "function"
            ? 42
            : 0
        "#,
    )?;
    ensure_value(&typed, &Value::Number(42.0))
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

    Err(format!("expected {expected}, got {actual}").into())
}
