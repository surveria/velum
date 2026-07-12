use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_method_this_for_member_and_computed_calls() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let camera = { value: 40, name: "front" };
        camera.read = function(delta) {
            return this.value + delta;
        };
        camera.write = function(value) {
            this.value = value;
            return this.read(0);
        };

        let first = camera.read(2);
        let second = camera["write"](42);
        let parenthesized = (camera.read)(0);
        print(first, second, parenthesized, camera.value);

        first === 42 &&
            second === 42 &&
            parenthesized === 42 &&
            camera.value === 42 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["42 42 42 42".to_owned()])
}

#[test]
fn applies_callee_strictness_to_direct_call_this() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let read = function() {
            return this;
        };
        let strictRead = function() {
            "use strict";
            return this;
        };
        let sloppy = read() === globalThis;
        let strict = strictRead() === undefined;
        print(sloppy, strict);
        sloppy && strict ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["true true".to_owned()])
}

#[test]
fn rejects_assignment_to_this_expression() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval("this = 1;") else {
        return Err("expected assignment to this to fail".into());
    };
    ensure_error_contains(&error, "invalid assignment target")
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

fn ensure_error_contains(error: &rs_quickjs::Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error containing '{expected}', got '{message}'").into())
}
