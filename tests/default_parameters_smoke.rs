use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn applies_default_parameter_when_argument_is_missing_or_undefined() -> TestResult {
    let value = eval(
        r"
        function pick(value = 41) {
            return value + 1;
        }
        pick() + pick(undefined) + pick(9);
        ",
    )?;
    ensure_value(&value, &Value::Number(94.0))
}

#[test]
fn default_parameter_can_read_previous_parameter_and_outer_binding() -> TestResult {
    let value = eval(
        r"
        let base = 5;
        function add(left, right = left + base) {
            return right;
        }
        add(7);
        ",
    )?;
    ensure_value(&value, &Value::Number(12.0))
}

#[test]
fn default_parameters_initialize_sequentially() -> TestResult {
    let value = eval(
        r"
        function chain(first = 39, second = first + 2, third = second + 1) {
            return third;
        }
        chain();
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn destructured_parameters_initialize_before_later_defaults() -> TestResult {
    let value = eval(
        r"
        function read({left, nested: [right]}, sum = left + right) {
            return sum;
        }
        read({left: 19, nested: [23]});
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn destructured_parameter_defaults_observe_source_order_and_tdz() -> TestResult {
    let value = eval(
        r"
        function ordered([first = 20, second = first + 1], result = second * 2) {
            return result;
        }
        function readsLater({value = later}, later = 1) {
            return value;
        }

        let laterError = false;
        try {
            readsLater({});
        } catch (error) {
            laterError = error instanceof ReferenceError;
        }
        ordered([]) === 42 && laterError;
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn rest_pattern_keeps_missing_positional_arguments_in_place() -> TestResult {
    let value = eval(
        r#"
        function collect(first, second, ...[head, tail]) {
            return String(first) + ":" + String(second) + ":" + String(head) + ":" + String(tail);
        }
        collect() + ";" + collect(1, 2, 3, 4);
        "#,
    )?;
    ensure_value(
        &value,
        &Value::from("undefined:undefined:undefined:undefined;1:2:3:4"),
    )
}

#[test]
fn default_parameter_tdz_rejects_self_and_later_reads() -> TestResult {
    let value = eval(
        r#"
        function readSelf(value = value) {
            return value;
        }
        function readLater(first = second, second = 1) {
            return first + second;
        }

        let selfError = "";
        let laterError = "";
        try {
            readSelf();
        } catch (error) {
            selfError = error.name + ":" + error.message + ":" + (error.constructor === ReferenceError);
        }
        try {
            readLater();
        } catch (error) {
            laterError = error.name + ":" + error.message + ":" + (error.constructor === ReferenceError);
        }

        selfError === "ReferenceError:'value' is not initialized:true" &&
            laterError === "ReferenceError:'second' is not initialized:true"
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn supports_default_parameter_trailing_comma_and_function_length() -> TestResult {
    let value = eval(
        r"
        function combine(left, right = 3,) {
            return left + right + combine.length;
        }
        combine(4);
        ",
    )?;
    ensure_value(&value, &Value::Number(8.0))
}

#[test]
fn rejects_duplicate_non_simple_parameters() -> TestResult {
    let sources = [
        "function duplicate(value, value = 1) {}",
        "function duplicate(value = 1, value) {}",
        "(function(value, value = 1) {})",
        "async function duplicate(value, value = 1) {}",
        "(async function(value, value = 1) {})",
        "async (value, value = 1) => value",
    ];

    for source in sources {
        ensure_parse_error_contains(source, "duplicate parameter name")?;
    }

    Ok(())
}

#[test]
fn rejects_strict_function_parameter_early_errors() -> TestResult {
    let sources = [
        "\"use strict\"; async function duplicate(value, value) {}",
        "async function duplicate(value, value) { \"use strict\"; }",
        "async function invalid(value = 1) { \"use strict\"; }",
        "\"use strict\"; async function eval() {}",
        "\"use strict\"; async function arguments() {}",
        "\"use strict\"; async function invalid(eval) {}",
        "\"use strict\"; async function invalid(arguments) {}",
        "\"use strict\"; (async function eval() {})",
        "\"use strict\"; async (eval) => eval",
        "function eval() { \"use strict\"; }",
        "function arguments() { \"use strict\"; }",
        "(function eval() { \"use strict\"; })",
        "(function arguments() { \"use strict\"; })",
    ];

    for source in sources {
        ensure_parse_error(source)?;
    }

    Ok(())
}

#[test]
fn async_function_uses_default_parameter_before_body() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r"
        async function answer(value = 40) {
            return value + 2;
        }
        let resolved = 0;
        answer(undefined).then(function(value) {
            resolved = value;
        });
        ",
    )?;
    let value = context.eval("resolved")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn async_declaration_name_uses_the_outer_await_context() -> TestResult {
    let value = eval("async function await() { return 42; } await instanceof Function")?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn async_default_parameter_tdz_rejects_returned_promise() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let bodyStarted = false;
        let rejected = "";

        async function task(value = value) {
            bodyStarted = true;
            return value;
        }

        task().then(function() {
            rejected = "resolved";
        }, function(error) {
            rejected = error.name + ":" + error.message + ":" + (error.constructor === ReferenceError);
        });
        "#,
    )?;
    let value = context.eval(
        r#"
        !bodyStarted && rejected === "ReferenceError:'value' is not initialized:true"
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn async_default_parameter_abrupt_completion_rejects_returned_promise() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let bodyStarted = false;
        let rejected = "";
        function thrower() {
            throw new Test262Error("boom");
        }

        async function task(value = thrower()) {
            bodyStarted = true;
            return value;
        }

        task().then(function() {
            rejected = "resolved";
        }, function(error) {
            rejected = error.message + ":" + (error.constructor === Test262Error);
        });
        "#,
    )?;
    let value = context.eval(
        r#"
        !bodyStarted && rejected === "boom:true"
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_parse_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    let rs_quickjs::Error::Parse { message, .. } = error else {
        return Err(format!("expected parse error for '{source}', got {error:?}").into());
    };
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected parse error containing '{expected}', got '{message}'").into())
}

fn ensure_parse_error(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    if matches!(error, rs_quickjs::Error::Parse { .. }) {
        return Ok(());
    }
    Err(format!("expected parse error for '{source}', got {error:?}").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
