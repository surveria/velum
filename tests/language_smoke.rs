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
    ensure_optional_value(context.get_global("x").as_ref(), &Value::Number(42.0))?;
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
    ensure_optional_value(context.get_global("value").as_ref(), &Value::Number(42.0))?;
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
    ensure_optional_value(context.get_global("first").as_ref(), &Value::Undefined)?;

    let extra = context.eval(
        r"
        let observed = 0;
        let pick = function(value) {
            return value;
        };
        pick(7, observed = 42);
        observed
        ",
    )?;
    ensure_value(&extra, &Value::Number(42.0))
}

#[test]
fn supports_function_return_statements() -> TestResult {
    expect_value(
        r"
        let choose = function() {
            if (true) {
                return 40 + 2;
            }
            return 0;
        };
        choose()
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        let stop = function() {
            return;
            42;
        };
        stop()
        ",
        &Value::Undefined,
    )?;

    expect_value(
        r"
        let wrapped = function() {
            try {
                return 42;
            } catch (error) {
                return 0;
            }
        };
        wrapped()
        ",
        &Value::Number(42.0),
    )?;

    let Err(error) = eval("return 1;") else {
        return Err("expected top-level return to fail".into());
    };
    ensure_error_contains(&error, "return statement outside function")
}

#[test]
fn supports_function_parameters_and_local_scope() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let global = 1;
        let add = function(left, right) {
            var local = left + right;
            return local;
        };
        let missing = function(value) {
            return value;
        };
        let bump = function(delta) {
            global = global + delta;
            return global;
        };
        let result = add(40, 2);
        let absent = missing();
        let total = bump(41);
        result
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("global").as_ref(), &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("result").as_ref(), &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("absent").as_ref(), &Value::Undefined)?;
    ensure_optional_value(context.get_global("total").as_ref(), &Value::Number(42.0))?;
    ensure_missing_global(context.get_global("local").as_ref(), "local")?;

    expect_value(
        r"
        let duplicate = function(value, value) {
            return value;
        };
        duplicate(1, 2)
        ",
        &Value::Number(2.0),
    )
}

#[test]
fn supports_escaping_closures() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let makeCounter = function(start) {
            var value = start;
            return function(delta) {
                value = value + delta;
                return value;
            };
        };
        let counter = makeCounter(40);
        let first = counter(1);
        let second = counter(1);
        second
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("first").as_ref(), &Value::Number(41.0))?;
    ensure_optional_value(context.get_global("second").as_ref(), &Value::Number(42.0))?;
    ensure_missing_global(context.get_global("value").as_ref(), "value")?;

    expect_value(
        r"
        let makeReader = function() {
            var value = 1;
            let read = function() {
                return value;
            };
            value = 42;
            return read;
        };
        let read = makeReader();
        read()
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        let makeCounter = function(start) {
            var value = start;
            return function() {
                value = value + 1;
                return value;
            };
        };
        let left = makeCounter(0);
        let right = makeCounter(40);
        left() + left() + right()
        ",
        &Value::Number(44.0),
    )?;

    expect_value(
        r"
        let outer = function(a) {
            return function(b) {
                return function(c) {
                    return a + b + c;
                };
            };
        };
        outer(20)(20)(2)
        ",
        &Value::Number(42.0),
    )?;

    let Err(error) = eval(
        r"
        let makeWriter = function() {
            const value = 1;
            return function() {
                value = 2;
            };
        };
        let write = makeWriter();
        write();
        ",
    ) else {
        return Err("expected captured const assignment to fail".into());
    };
    ensure_error_contains(&error, "assignment to constant")
}

#[test]
fn supports_object_literals_and_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let camera = {
            name: "front-door",
            count: 40,
            nested: { value: 2 },
            duplicate: 1,
            duplicate: 41,
        };
        let assigned = camera.count = camera.count + camera.nested.value;
        camera.extra = camera.duplicate + 1;
        print(camera.name, camera.missing);
        assigned + camera.extra - 42
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["front-door undefined".to_owned()])?;
    ensure_global_type(context.get_global("camera").as_ref(), "object", "camera")?;

    expect_value(
        r"
        let shared = {};
        let same = shared === shared;
        let different = shared === {};
        same && !different ? 42 : 0
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        let make = function() {
            let state = { value: 40 };
            return function() {
                state.value = state.value + 1;
                return state.value;
            };
        };
        let next = make();
        next();
        next()
        ",
        &Value::Number(42.0),
    )?;

    let Err(error) = eval("let value = 1; value.name = 2;") else {
        return Err("expected property assignment on non-object to fail".into());
    };
    ensure_error_contains(&error, "property assignment")
}

#[test]
fn supports_computed_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let camera = {
            name: "front-door",
            count: 40,
            nested: { value: 2 },
        };
        let key = "count";
        let assigned = camera[key] = camera[key] + camera["nested"].value;
        camera[1] = assigned;
        camera[true] = camera["1"];
        print(camera["name"], camera["missing"]);
        assigned + camera[true] - 42
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["front-door undefined".to_owned()])?;

    expect_value(
        r#"
        let order = "";
        let object = {};
        let key = function() {
            order = order + "k";
            return "value";
        };
        let payload = function() {
            order = order + "v";
            return 42;
        };
        object[key()] = payload();
        order + ":" + object.value
        "#,
        &Value::String("kv:42".to_owned()),
    )?;

    expect_value(
        r#"
        try {
            missing = missing;
        } catch (error) {
            error["name"] + ":" + error["message"]
        }
        "#,
        &Value::String("ReferenceError:'missing' is not defined".to_owned()),
    )?;

    let Err(error) = eval("let value = 1; value['name'] = 2;") else {
        return Err("expected computed property assignment on non-object to fail".into());
    };
    ensure_error_contains(&error, "property assignment")
}

#[test]
fn supports_array_literals_and_index_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [40, 1, 2];
        let assigned = values[1] = values[0] + values[2];
        values[3] = assigned;
        values["01"] = 7;
        print(values.length, values[2], values[9]);
        print(values["01"], values.length);
        values.length + values[3] - 4
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["4 2 undefined".to_owned(), "7 4".to_owned()],
    )?;
    ensure_global_type(context.get_global("values").as_ref(), "object", "values")?;

    expect_value("let empty = []; empty.length", &Value::Number(0.0))?;
    expect_value(
        "let trailing = [40, 2,]; trailing.length",
        &Value::Number(2.0),
    )?;
    expect_value(
        r"
        let make = function() {
            let values = [40];
            return function() {
                values[0] = values[0] + 1;
                return values[0];
            };
        };
        let next = make();
        next();
        next()
        ",
        &Value::Number(42.0),
    )?;

    let Err(error) = eval("let values = [1, 2]; values.length = 1;") else {
        return Err("expected array length assignment to fail".into());
    };
    ensure_error_contains(&error, "array length assignment")
}

#[test]
fn supports_assert_throws_and_reference_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        var first, second = 40, third = second + 2;
        let caught_name;
        let caught_message;

        assert.throws(ReferenceError, function() {
            absent = absent;
        });

        try {
            missing = missing;
        } catch (error) {
            caught_name = error.name;
            caught_message = error.message;
            print(error);
        }

        third
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("first").as_ref(), &Value::Undefined)?;
    ensure_optional_value(
        context.get_global("caught_name").as_ref(),
        &Value::String("ReferenceError".to_owned()),
    )?;
    ensure_optional_value(
        context.get_global("caught_message").as_ref(),
        &Value::String("'missing' is not defined".to_owned()),
    )?;
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
fn supports_error_object_properties() -> TestResult {
    expect_value(
        r#"
        try {
            missing = missing;
        } catch (error) {
            error.name + ":" + error.message
        }
        "#,
        &Value::String("ReferenceError:'missing' is not defined".to_owned()),
    )?;

    expect_value(
        r#"
        try {
            throw new Test262Error("bad value");
        } catch (error) {
            error.name + ":" + error.message
        }
        "#,
        &Value::String("Test262Error:bad value".to_owned()),
    )
}

#[test]
fn supports_standard_error_constructors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let value = 0;
        let plain = Error("plain", value = value + 1);
        let typed = new TypeError("typed");
        let syntax = SyntaxError("syntax");

        assert.throws(TypeError, function() {
            throw new TypeError("boom");
        }, "TypeError should match");

        assert.throws(Error, function() {
            throw new RangeError("range");
        });

        if (TypeError.prototype.constructor === TypeError) {
            value = value + 20;
        }
        if (SyntaxError.name === "SyntaxError" && SyntaxError.length === 1) {
            value = value + 21;
        }

        print(
            plain.name,
            plain.message,
            typed.name,
            typed.message,
            syntax.name,
            syntax.message
        );
        value
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["Error plain TypeError typed SyntaxError syntax".to_owned()],
    )
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
        context.get_global("marker").as_ref(),
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

    let limits = RuntimeLimits {
        max_objects: 0,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let Err(error) = context.eval("let value = {};") else {
        return Err("expected object count limit to fail".into());
    };
    ensure_error_kind(&error, "resource limit")?;

    let limits = RuntimeLimits {
        max_object_properties: 1,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let Err(error) = context.eval("let value = { first: 1, second: 2 };") else {
        return Err("expected object property count limit to fail".into());
    };
    ensure_error_kind(&error, "resource limit")
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

fn ensure_missing_global(actual: Option<&Value>, name: &str) -> TestResult {
    if actual.is_none() {
        return Ok(());
    }

    Err(format!("expected global '{name}' to be missing, got {actual:?}").into())
}

fn ensure_global_type(actual: Option<&Value>, expected: &str, name: &str) -> TestResult {
    let Some(actual) = actual else {
        return Err(format!("expected global '{name}' to exist").into());
    };
    if actual.type_name() == expected {
        return Ok(());
    }

    Err(format!(
        "expected global '{name}' to have type '{expected}', got '{}'",
        actual.type_name()
    )
    .into())
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
