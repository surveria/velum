use velum::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn reuses_registered_native_functions_for_repeated_builtin_access() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.eval(
        r"
        Array;
        Array.prototype.push;
        Boolean;
        Error;
        EvalError;
        JSON.parse;
        Math.abs;
        Math.max;
        Number;
        Object;
        Object.keys;
        String;
        TypeError;
        42
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))?;
    let first_usage = vm.resource_usage();

    let value = vm.eval(
        r"
        Array;
        Array.prototype.push;
        Boolean;
        Error;
        EvalError;
        JSON.parse;
        Math.abs;
        Math.max;
        Number;
        Object;
        Object.keys;
        String;
        TypeError;
        42
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))?;
    let second_usage = vm.resource_usage();

    ensure_usize(
        second_usage.native_function_count,
        first_usage.native_function_count,
    )
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
