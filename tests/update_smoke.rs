use velum::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_prefix_and_postfix_update_expressions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let value = 40;
        let first = value++;
        let second = ++value;
        let third = value--;
        let fourth = --value;
        print(first, second, third, fourth, value);

        let sensor = { count: 1 };
        let propOld = sensor.count++;
        let propNew = ++sensor.count;
        print(propOld, propNew, sensor.count);

        let values = [1, 2];
        let index = 0;
        let cellOld = values[index]++;
        let cellNew = ++values[1];
        print(cellOld, cellNew, values[0], values[1]);

        let total = 0;
        for (let step = 0; step < 4; step++) {
            total = total + step;
        }
        let down = 2;
        while (down--) {}
        print(total, down);

        first === 40 &&
            second === 42 &&
            third === 42 &&
            fourth === 40 &&
            value === 40 &&
            propOld === 1 &&
            propNew === 3 &&
            sensor.count === 3 &&
            cellOld === 1 &&
            cellNew === 3 &&
            values[0] === 2 &&
            values[1] === 3 &&
            total === 6 &&
            down === -1
        ",
    )?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(
        context.output(),
        &[
            "40 42 42 40 40".to_owned(),
            "1 3 3".to_owned(),
            "1 3 2 3".to_owned(),
            "6 -1".to_owned(),
        ],
    )
}

#[test]
fn rejects_invalid_update_targets_and_const_bindings() -> TestResult {
    ensure_error_contains("++1", "invalid update target")?;
    ensure_error_contains("let value = 1; (value + 1)++", "invalid update target")?;
    ensure_error_contains("const fixed = 1; fixed++", "assignment to constant")
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
