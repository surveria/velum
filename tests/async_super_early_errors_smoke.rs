use velum::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn rejects_super_references_in_async_function_bodies() -> TestResult {
    let sources = [
        "async function task() { super(); }",
        "async function task() { super.value; }",
        "(async function() { super(); })",
        "(async function() { super.value; })",
        "async () => { super(); }",
        "async () => { super.value; }",
    ];

    for source in sources {
        ensure_parse_error_contains(source, "is only valid inside")?;
    }

    Ok(())
}

#[test]
fn rejects_super_references_in_async_parameter_defaults() -> TestResult {
    let sources = [
        "async function task(value = super()) {}",
        "async function task(value = super.value) {}",
        "(async function(value = super()) {})",
        "(async function(value = super.value) {})",
        "async (value = super()) => value",
        "async (value = super.value) => value",
    ];

    for source in sources {
        ensure_parse_error_contains(source, "is only valid inside")?;
    }

    Ok(())
}

#[test]
fn rejects_super_as_binding_identifier() -> TestResult {
    let sources = [
        "async function super() {}",
        "async function task(super) {}",
        "(async function(super) {})",
        "async super => super",
    ];

    for source in sources {
        ensure_parse_error_contains(source, "super is not a valid binding identifier")?;
    }

    Ok(())
}

#[test]
fn rejects_super_as_function_expression_name() -> TestResult {
    ensure_parse_error_contains(
        "(async function super() {})",
        "super is not a valid identifier",
    )
}

#[test]
fn keeps_super_property_names_allowed() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        let object = { super: 41 };
        object.super + 1
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_parse_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    match error {
        Error::Parse { message, .. } if message.contains(expected) => Ok(()),
        other => Err(format!("expected parse error containing '{expected}', got {other:?}").into()),
    }
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
