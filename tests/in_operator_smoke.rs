use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_in_operator_for_objects_and_arrays() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { present: 1, empty: undefined };
        let present = "present" in object;
        let empty = "empty" in object;
        let absent = "absent" in object;
        print(present, empty, absent);

        delete object.present;
        let deleted = "present" in object;
        print(deleted);

        let values = [undefined, 2];
        values[3] = 4;
        let hasZero = 0 in values;
        let hasOne = "1" in values;
        let hasTwo = 2 in values;
        let hasThree = 3 in values;
        let hasLength = "length" in values;
        print(hasZero, hasOne, hasTwo, hasThree, hasLength);

        let key = "slot";
        let bag = { slot: 42 };
        let precedence = key in bag === true;
        print(key in bag, ("slot") in bag, precedence);

        present === true &&
            empty === true &&
            absent === false &&
            deleted === false &&
            hasZero === true &&
            hasOne === true &&
            hasTwo === false &&
            hasThree === true &&
            hasLength === true &&
            precedence === true
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(
        context.output(),
        &[
            "true true false".to_owned(),
            "false".to_owned(),
            "true true false true true".to_owned(),
            "true true true".to_owned(),
        ],
    )
}

#[test]
fn rejects_in_operator_for_non_object_rhs() -> TestResult {
    ensure_error_contains(r#""slot" in null"#, "operator 'in'")?;
    ensure_error_contains(r#""slot" in 1"#, "operator 'in'")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    error_contains(&error, expected)
}

fn error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
