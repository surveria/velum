use rs_quickjs::{Error, Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn evaluates_arithmetic_with_precedence() -> TestResult {
    expect_value("1 + 2 * 3 - 4 / 2", &Value::Number(5.0))
}

#[test]
fn evaluates_bindings_and_assignment() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval("let x = 40; x = x + 2; x")?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("x"), &Value::Number(42.0))?;
    Ok(())
}

#[test]
fn keeps_const_bindings_immutable() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval("const x = 1; x = 2") else {
        return Err("expected const assignment to fail".into());
    };
    ensure_error_kind(&error, "runtime")?;
    Ok(())
}

#[test]
fn supports_strings_and_host_print() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(r#"let name = "camera"; print("hello", name); "id-" + 7"#)?;

    ensure_value(&value, &Value::String("id-7".to_owned()))?;
    ensure_output(context.output(), &["hello camera".to_owned()])?;
    Ok(())
}

#[test]
fn supports_boolean_function_conversion() -> TestResult {
    expect_value("Boolean()", &Value::Bool(false))?;
    expect_value("Boolean(false)", &Value::Bool(false))?;
    expect_value("Boolean(0)", &Value::Bool(false))?;
    expect_value(r#"Boolean("")"#, &Value::Bool(false))?;
    expect_value("Boolean(null)", &Value::Bool(false))?;
    expect_value("Boolean(undefined)", &Value::Bool(false))?;
    expect_value("Boolean(true)", &Value::Bool(true))?;
    expect_value("Boolean(1)", &Value::Bool(true))?;
    expect_value(r#"Boolean("camera")"#, &Value::Bool(true))
}

#[test]
fn supports_basic_var_hoisting() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        print(value);
        var value = 40;
        value = value + 2;
        var value;
        value
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("value"), &Value::Number(42.0))?;
    ensure_output(context.output(), &["undefined".to_owned()])?;

    let Err(error) = eval("let lexical = 1; var lexical;") else {
        return Err("expected var and lexical redeclaration conflict".into());
    };
    ensure_error_kind(&error, "runtime")
}

#[test]
fn short_circuits_logical_operators() -> TestResult {
    expect_value("false && missing", &Value::Bool(false))?;
    expect_value(r#""ok" || missing"#, &Value::String("ok".to_owned()))
}

#[test]
fn supports_conditional_and_bitwise_and() -> TestResult {
    expect_value("true ? 42 : missing", &Value::Number(42.0))?;
    expect_value("false ? missing : 42", &Value::Number(42.0))?;
    expect_value("(true ? 5 : 0) & 3", &Value::Number(1.0))?;
    expect_value("-1 & 1", &Value::Number(1.0))?;
    expect_value("4294967297 & 3", &Value::Number(1.0))?;
    expect_value(
        r"
        let value = false ? missing : 40;
        value = value + (((value === 40) & true) ? 2 : 0);
        value
        ",
        &Value::Number(42.0),
    )?;

    expect_value(r#""camera" & 1"#, &Value::Number(0.0))
}

#[test]
fn supports_function_expressions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let value = 0;
        let update = function() {
            value = value + 20;
        };
        let first = update();
        update();
        value = value + 2;
        value
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("first"), &Value::Undefined)?;

    let Err(error) = eval(
        r"
        let update = function() {
            1;
        };
        update(1);
        ",
    ) else {
        return Err("expected function arguments to fail".into());
    };
    ensure_error_kind(&error, "runtime")
}

#[test]
fn supports_assert_throws_and_reference_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        var first, second = 40, third = second + 2;

        assert.throws(ReferenceError, function() {
            absent = absent;
        });

        try {
            missing = missing;
        } catch (error) {
            print(error);
        }

        third
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("first"), &Value::Undefined)?;
    ensure_output(
        context.output(),
        &["ReferenceError: 'missing' is not defined".to_owned()],
    )?;

    let Err(error) = eval(
        r"
        assert.throws(ReferenceError, function() {
            1;
        });
        ",
    ) else {
        return Err("expected assert.throws without an exception to fail".into());
    };
    ensure_error_contains(&error, "no exception was thrown")?;

    let Err(error) = eval("missing") else {
        return Err("expected missing identifier to fail".into());
    };
    ensure_error_contains(&error, "ReferenceError: 'missing' is not defined")
}

#[test]
fn evaluates_if_blocks_and_throw_statements() -> TestResult {
    expect_value(
        r#"
        let value = 1;
        if (value === 1) {
            value = value + 41;
        } else {
            throw new Test262Error("unreachable");
        }
        value
        "#,
        &Value::Number(42.0),
    )?;

    let Err(error) = eval(r#"throw new Test262Error("expected failure")"#) else {
        return Err("expected throw statement to fail".into());
    };
    ensure_error_kind(&error, "runtime")
}

#[test]
fn catches_thrown_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let marker = "outer";
        let value = 0;
        try {
            throw "boom";
            value = 1;
        } catch (marker) {
            print(marker);
            value = 42;
        }
        if (marker !== "outer") {
            throw new Test262Error("catch binding leaked");
        }
        value
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(
        context.get_global("marker"),
        &Value::String("outer".to_owned()),
    )?;
    ensure_output(context.output(), &["boom".to_owned()])?;

    let Err(error) = eval(
        r#"
        try {
            throw "first";
        } catch (error) {
            throw "second";
        }
        "#,
    ) else {
        return Err("expected rethrow from catch block to fail".into());
    };
    ensure_error_kind(&error, "runtime")
}

#[test]
fn enforces_resource_limits() -> TestResult {
    let limits = RuntimeLimits {
        max_source_len: 8,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let Err(error) = context.eval("let x = 10;") else {
        return Err("expected resource limit to fail".into());
    };
    ensure_error_kind(&error, "resource limit")?;
    Ok(())
}

fn expect_value(source: &str, expected: &Value) -> TestResult {
    let actual = eval(source)?;
    ensure_value(&actual, expected)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_optional_value(actual: Option<&Value>, expected: &Value) -> TestResult {
    if actual == Some(expected) {
        return Ok(());
    }

    Err(format!("expected global value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_error_kind(error: &Error, expected: &str) -> TestResult {
    let matches = matches!(
        (error, expected),
        (Error::Runtime { .. }, "runtime") | (Error::ResourceLimit { .. }, "resource limit")
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
