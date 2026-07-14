use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_global_numeric_constants_as_immutable_bindings() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let nanBefore = NaN;
        let infinityBefore = Infinity;
        let deleteNaN = delete NaN;
        let deleteInfinity = delete Infinity;
        let deleteObject = delete Object;
        let deleteMissing = delete missingGlobalName;

        let shadow = 0;
        {
          let NaN = 40;
          let Infinity = 2;
          shadow = NaN + Infinity;
        }

        print(typeof NaN, NaN !== NaN, Infinity > 1e300, -Infinity < -1e300);
        print(deleteNaN, deleteInfinity, deleteObject, deleteMissing);

        typeof nanBefore === "number" &&
          nanBefore !== nanBefore &&
          infinityBefore === Infinity &&
          Infinity > 1e300 &&
          -Infinity < -1e300 &&
          deleteNaN === false &&
          deleteInfinity === false &&
            deleteObject === true &&
          deleteMissing === true &&
          shadow === 42
            ? 42
            : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["number true true true", "false false true true"],
    )?;

    let value = context.eval("NaN = 7; Infinity += 1; NaN !== NaN && Infinity > 1e300")?;
    ensure_value(&value, &Value::Bool(true))?;

    let Err(error) = context.eval(r#""use strict"; NaN = 7"#) else {
        return Err("expected strict assignment to NaN to fail".into());
    };
    ensure_error_contains(&error, "assignment to constant 'NaN'")?;

    let Err(error) = context.eval(r#""use strict"; Infinity += 1"#) else {
        return Err("expected strict compound assignment to Infinity to fail".into());
    };
    ensure_error_contains(&error, "assignment to constant 'Infinity'")
}

#[test]
fn compiled_numeric_constants_use_guarded_direct_loads() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        NaN !== NaN &&
          Infinity > 1e300 &&
          -Infinity < -1e300 &&
          typeof NaN === "number"
            ? 42
            : 0
        "#,
    )?;
    let global_bindings = vm.resource_usage().global_bindings;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().global_bindings, global_bindings)?;

    let value = vm.eval(
        r"
        {
          let NaN = 7;
          let Infinity = 8;
          NaN + Infinity;
        }
        ",
    )?;
    ensure_value(&value, &Value::Number(15.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected {expected}, got {actual}").into())
}

fn ensure_error_contains(error: &rs_quickjs::Error, needle: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(needle) {
        return Ok(());
    }

    Err(format!("expected error containing '{needle}', got '{message}'").into())
}
