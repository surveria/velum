use rs_quickjs::{Error, Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn supports_while_statements() -> TestResult {
    expect_value(
        r"
        let values = [10, 20, 10, 2];
        let index = 0;
        let total = 0;
        while (index < values.length) {
            total = total + values[index];
            index = index + 1;
        }
        total
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        let index = 0;
        while (index < 3) {
            index = index + 1;
        }
        ",
        &Value::Number(3.0),
    )?;

    expect_value(
        r"
        while (false) {
            var hoisted = 42;
        }
        hoisted
        ",
        &Value::Undefined,
    )
}

#[test]
fn propagates_while_completion() -> TestResult {
    expect_value(
        r"
        let pick = function() {
            let index = 0;
            while (index < 4) {
                index = index + 1;
                if (index === 2) {
                    return 42;
                }
            }
            return 0;
        };
        pick()
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r#"
        let caught = "none";
        try {
            while (true) {
                throw "boom";
            }
        } catch (error) {
            caught = error;
        }
        caught
        "#,
        &Value::String("boom".to_owned()),
    )
}

#[test]
fn supports_break_and_continue() -> TestResult {
    expect_value(
        r"
        let values = [20, 1, 22, 100];
        let index = 0;
        let total = 0;
        while (index < values.length) {
            if (index === 1) {
                index = index + 1;
                continue;
            }
            if (index === 3) {
                break;
            }
            total = total + values[index];
            index = index + 1;
        }
        total
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        let index = 0;
        let total = 0;
        while (index < 5) {
            index = index + 1;
            try {
                if (index === 2) {
                    continue;
                }
                if (index === 4) {
                    break;
                }
            } catch (error) {
                total = 0;
            }
            total = total + index;
        }
        total
        ",
        &Value::Number(4.0),
    )
}

#[test]
fn rejects_break_and_continue_outside_loops() -> TestResult {
    let Err(error) = eval("break;") else {
        return Err("expected top-level break to fail".into());
    };
    ensure_error_contains(&error, "break statement outside loop")?;

    let Err(error) = eval("continue;") else {
        return Err("expected top-level continue to fail".into());
    };
    ensure_error_contains(&error, "continue statement outside loop")?;

    let Err(error) = eval("let fail = function() { break; }; fail();") else {
        return Err("expected function-local break outside a loop to fail".into());
    };
    ensure_error_contains(&error, "break statement outside loop")
}

#[test]
fn limits_infinite_while_loops() -> TestResult {
    let limits = RuntimeLimits {
        max_runtime_steps: 16,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let Err(error) = context.eval("while (true) {}") else {
        return Err("expected infinite while loop to hit runtime step limit".into());
    };
    ensure_error_kind(&error, "resource limit")?;
    ensure_error_contains(&error, "runtime steps")
}

fn expect_value(source: &str, expected: &Value) -> TestResult {
    let actual = eval(source)?;
    if &actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_error_kind(error: &Error, expected: &str) -> TestResult {
    let matches = matches!(
        (error, expected),
        (Error::ResourceLimit { .. }, "resource limit")
    );
    if matches {
        return Ok(());
    }
    Err(format!("expected {expected} error, got {error:?}").into())
}

fn ensure_error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
