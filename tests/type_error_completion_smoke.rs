use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn catches_non_callable_call_type_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let caught = "";
        try {
            ({})();
        } catch (error) {
            caught = error.name + ":" + error.message;
        }

        assert.throws(TypeError, function() {
            ({}).missing();
        });

        caught
        "#,
    )?;

    ensure_string_starts_with(&value, "TypeError:")?;
    ensure_string_contains(&value, "not callable")
}

#[test]
fn catches_non_constructable_function_type_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        async function task() {}
        let caught = "";

        try {
            new task();
        } catch (error) {
            caught = error.name + ":" + error.message;
        }

        assert.throws(TypeError, function() {
            new task();
        });

        let AsyncFunction = Object.getPrototypeOf(task).constructor;
        let generated = AsyncFunction();
        assert.throws(TypeError, function() {
            new generated();
        });

        caught
        "#,
    )?;

    ensure_string_starts_with(&value, "TypeError:")?;
    ensure_string_contains(&value, "not a constructor")
}

#[test]
fn catches_function_prototype_call_and_bind_type_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        assert.throws(TypeError, function() {
            Function.prototype.call.call({}, null);
        });

        assert.throws(TypeError, function() {
            Function.prototype.bind.call({}, null);
        });

        42
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn catches_throw_from_called_js_function() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function fail() {
            throw new TypeError("from callee");
        }
        let caught = "";

        try {
            fail();
        } catch (error) {
            caught = error.name + ":" + error.message;
        }

        assert.throws(TypeError, function() {
            fail();
        });

        caught
        "#,
    )?;

    ensure_string_starts_with(&value, "TypeError:")?;
    ensure_string_contains(&value, "from callee")
}

#[test]
fn catches_promise_constructor_type_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let caught = "";

        try {
            new Promise();
        } catch (error) {
            caught = error.name + ":" + error.message;
        }

        assert.throws(TypeError, function() {
            new Promise({});
        });

        caught
        "#,
    )?;

    ensure_string_starts_with(&value, "TypeError:")?;
    ensure_string_contains(&value, "requires an executor")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_string_starts_with(value: &Value, expected: &str) -> TestResult {
    let actual = string_value(value)?;
    if actual.starts_with(expected) {
        return Ok(());
    }

    Err(format!("expected string prefix '{expected}', got '{actual}'").into())
}

fn ensure_string_contains(value: &Value, expected: &str) -> TestResult {
    let actual = string_value(value)?;
    if actual.contains(expected) {
        return Ok(());
    }

    Err(format!("expected string containing '{expected}', got '{actual}'").into())
}

fn string_value(value: &Value) -> std::result::Result<&str, Box<dyn std::error::Error>> {
    match value {
        Value::String(actual) => Ok(actual.as_str()),
        Value::HeapString(actual) => Ok(actual.as_str()),
        _ => Err(format!("expected string value, got {value:?}").into()),
    }
}
