use rs_quickjs::{Runtime, Value};

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
          deleteObject === false &&
          deleteMissing === true &&
          shadow === 42
            ? 42
            : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["number true true true", "false false false true"],
    )?;

    let Err(error) = context.eval("NaN = 7") else {
        return Err("expected assigning NaN to fail".into());
    };
    ensure_error_contains(&error, "assignment to constant 'NaN'")?;

    let Err(error) = context.eval("Infinity += 1") else {
        return Err("expected compound-assigning Infinity to fail".into());
    };
    ensure_error_contains(&error, "assignment to constant 'Infinity'")
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

fn ensure_error_contains(error: &rs_quickjs::Error, needle: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(needle) {
        return Ok(());
    }

    Err(format!("expected error containing '{needle}', got '{message}'").into())
}
