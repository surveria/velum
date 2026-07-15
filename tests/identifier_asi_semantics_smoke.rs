use velum::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn undefined_is_an_ordinary_shadowable_identifier() -> TestResult {
    ensure_value(
        &eval(
            r"
            function parameter(undefined) { return undefined; }
            function local() { var undefined = 41; return undefined + 1; }
            parameter(42) === local() &&
                (function (u\u006edef) { return u\u006edef; })(true)
            ",
        )?,
        &Value::Bool(true),
    )?;

    ensure_value(
        &eval(
            r"
            let caught = 0;
            try { throw 42; } catch (undefined) { caught = undefined; }
            caught
            ",
        )?,
        &Value::Number(42.0),
    )?;

    ensure_value(
        &eval("with ({ undefined: 42 }) undefined")?,
        &Value::Number(42.0),
    )
}

#[test]
fn unresolved_undefined_survives_cross_script_closures() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r"
        function readUndefined() { return undefined; }
        function bindUndefined(callback) { return callback.bind(undefined); }
        ",
    )?;
    ensure_value(&context.eval("readUndefined()")?, &Value::Undefined)?;
    ensure_value(
        &context.eval(r#"bindUndefined(function () { "use strict"; return this; })()"#)?,
        &Value::Undefined,
    )
}

#[test]
fn global_undefined_has_the_standard_constant_descriptor() -> TestResult {
    ensure_value(
        &eval(
            r#"
            let descriptor = Object.getOwnPropertyDescriptor(globalThis, "undefined");
            "undefined" in globalThis &&
                descriptor.value === undefined &&
                descriptor.writable === false &&
                descriptor.enumerable === false &&
                descriptor.configurable === false
            "#,
        )?,
        &Value::Bool(true),
    )?;

    ensure_value(
        &eval(
            r#"
            undefined = 42;
            let strictCaught = false;
            try {
                (function () { "use strict"; undefined = 42; })();
            } catch (error) {
                strictCaught = error instanceof TypeError;
            }
            undefined === void 0 && strictCaught && delete undefined === false
            "#,
        )?,
        &Value::Bool(true),
    )
}

#[test]
fn return_obeys_the_line_terminator_restriction() -> TestResult {
    ensure_value(
        &eval(
            r"
            function value() {
                return
                42;
            }
            value()
            ",
        )?,
        &Value::Undefined,
    )?;

    ensure_parse_error("function invalid() { return 1 2; }")?;
    ensure_parse_error("return 42;")
}

#[test]
fn throw_rejects_line_terminators_and_missing_statement_terminators() -> TestResult {
    ensure_parse_error("throw\nnew Error('invalid');")?;
    ensure_parse_error("throw new Error('invalid') 42;")
}

fn ensure_parse_error(source: &str) -> TestResult {
    let Err(error) = eval(source) else {
        return Err("expected source to fail during parsing".into());
    };
    if matches!(error, Error::Parse { .. }) {
        return Ok(());
    }
    Err(format!("expected parse error, got {error}").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
