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
fn preserves_if_statement_completion_values() -> TestResult {
    expect_value(
        r"
        42;
        if (false) {
            1;
        }
        ",
        &Value::Undefined,
    )?;

    expect_value(
        r"
        if (true) {
            42;
        } else {
            1;
        }
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn supports_do_while_statements() -> TestResult {
    expect_value(
        r"
        let index = 0;
        let total = 0;
        do {
            index = index + 1;
            total = total + index;
        } while (index < 6);
        total
        ",
        &Value::Number(21.0),
    )?;

    expect_value(
        r"
        let ran = 0;
        do {
            ran = ran + 1;
        } while (false);
        ran
        ",
        &Value::Number(1.0),
    )?;

    expect_value(
        r"
        do {
            var hoisted = 42;
        } while (false);
        hoisted
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn propagates_do_while_control_flow() -> TestResult {
    expect_value(
        r"
        let index = 0;
        let total = 0;
        do {
            index = index + 1;
            if (index === 2) {
                continue;
            }
            if (index === 5) {
                break;
            }
            total = total + index;
        } while (index < 8);
        total
        ",
        &Value::Number(8.0),
    )?;

    expect_value(
        r"
        let pick = function() {
            let index = 0;
            do {
                index = index + 1;
                if (index === 2) {
                    return 42;
                }
            } while (index < 4);
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
            do {
                throw "boom";
            } while (false);
        } catch (error) {
            caught = error;
        }
        caught
        "#,
        &Value::String("boom".to_owned()),
    )
}

#[test]
fn supports_labeled_break_statements() -> TestResult {
    expect_value(
        r"
        let value = 0;
        outer: do {
            inner: do {
                value = 42;
                break outer;
                value = 1;
            } while (false);
            value = 2;
        } while (false);
        value
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        let index = 0;
        done: {
            do {
                index = index + 1;
                if (index === 3) {
                    break done;
                }
            } while (true);
            index = 100;
        }
        index
        ",
        &Value::Number(3.0),
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
fn supports_for_statements() -> TestResult {
    expect_value(
        r"
        let values = [10, 20, 10, 2];
        let total = 0;
        for (let index = 0; index < values.length; index = index + 1) {
            total = total + values[index];
        }
        total
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        let values = [20, 1, 22, 100];
        let index = 0;
        let total = 0;
        for (;;) {
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
        let visited = 0;
        for (let index = 0; index < 4; index = index + 1) {
            if (index === 1) {
                continue;
            }
            visited = visited + index;
        }
        visited
        ",
        &Value::Number(5.0),
    )?;

    expect_value(
        r"
        for (var hoisted = 42; false;) {
            hoisted = 0;
        }
        hoisted
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn propagates_for_completion() -> TestResult {
    expect_value(
        r"
        let pick = function() {
            for (let index = 0; index < 4; index = index + 1) {
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
            for (;;) {
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
fn limits_infinite_for_loops() -> TestResult {
    let limits = RuntimeLimits {
        max_runtime_steps: 16,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let Err(error) = context.eval("for (;;) {}") else {
        return Err("expected infinite for loop to hit runtime step limit".into());
    };
    ensure_error_kind(&error, "resource limit")?;
    ensure_error_contains(&error, "runtime steps")
}

#[test]
fn supports_switch_statements() -> TestResult {
    expect_value(
        r#"
        let value = "camera";
        let total = 0;
        switch (value) {
            case "sensor":
                total = 1;
                break;
            case "camera":
                total = total + 20;
            case "lens":
                total = total + 22;
                break;
            default:
                total = 0;
        }
        total
        "#,
        &Value::Number(42.0),
    )?;

    expect_value(
        r#"
        let selected = "none";
        switch (2) {
            case 1:
                selected = "one";
                break;
            default:
                selected = "default";
                break;
            case 2:
                selected = "two";
                break;
        }
        selected
        "#,
        &Value::String("two".to_owned()),
    )?;

    expect_value(
        r#"
        let total = 0;
        switch ("missing") {
            case "camera":
                total = 1;
                break;
            default:
                total = 20;
            case "lens":
                total = total + 22;
                break;
        }
        total
        "#,
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        switch (0) {
            case 1:
                var hoisted = 42;
        }
        hoisted
        ",
        &Value::Undefined,
    )
}

#[test]
fn propagates_switch_completion() -> TestResult {
    expect_value(
        r"
        let pick = function(value) {
            switch (value) {
                case 1:
                    return 42;
                default:
                    return 0;
            }
        };
        pick(1)
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r#"
        let caught = "none";
        try {
            switch (1) {
                case 1:
                    throw "boom";
            }
        } catch (error) {
            caught = error;
        }
        caught
        "#,
        &Value::String("boom".to_owned()),
    )?;

    expect_value(
        r"
        let total = 0;
        switch (1) {
            case 1:
                total = 42;
                break;
                total = 0;
        }
        total
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn propagates_switch_continue_to_outer_loop() -> TestResult {
    expect_value(
        r"
        let total = 0;
        for (let index = 0; index < 5; index = index + 1) {
            switch (index) {
                case 1:
                    continue;
                case 3:
                    break;
                default:
                    total = total + index;
            }
            total = total + 10;
        }
        total
        ",
        &Value::Number(46.0),
    )
}

#[test]
fn rejects_invalid_switch_control_flow() -> TestResult {
    let Err(error) = eval("switch (1) { case 1: continue; }") else {
        return Err("expected top-level switch continue to fail".into());
    };
    ensure_error_contains(&error, "continue statement outside loop")?;

    let Err(error) = eval("switch (1) { default: break; default: break; }") else {
        return Err("expected duplicate default labels to fail".into());
    };
    ensure_error_contains(&error, "multiple defaults")
}

#[test]
fn supports_try_finally_statements() -> TestResult {
    expect_value(
        r"
        let value = 0;
        try {
            value = 20;
        } finally {
            value = value + 22;
        }
        value
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r#"
        let value = 0;
        try {
            throw "caught";
        } catch (error) {
            if (error === "caught") {
                value = 20;
            }
        } finally {
            value = value + 22;
        }
        value
        "#,
        &Value::Number(42.0),
    )?;

    expect_value(
        r#"
        let value = 0;
        try {
            try {
                throw "boom";
            } finally {
                value = 42;
            }
        } catch (error) {
            value = value;
        }
        value
        "#,
        &Value::Number(42.0),
    )?;

    expect_value(
        r"
        try {
            42;
        } finally {
            0;
        }
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn propagates_try_finally_completion() -> TestResult {
    expect_value(
        r"
        let pick = function() {
            try {
                return 1;
            } finally {
                return 42;
            }
        };
        pick()
        ",
        &Value::Number(42.0),
    )?;

    expect_value(
        r#"
        let caught = "none";
        try {
            try {
                throw "try";
            } finally {
                throw "finally";
            }
        } catch (error) {
            caught = error;
        }
        caught
        "#,
        &Value::String("finally".to_owned()),
    )?;

    expect_value(
        r"
        let value = 0;
        while (true) {
            try {
                value = 42;
            } finally {
                break;
            }
            value = 0;
        }
        value
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn supports_omitted_catch_binding() -> TestResult {
    expect_value(
        r#"
        let marker = "outer";
        let value = 0;
        try {
            throw "boom";
        } catch {
            let marker = "inner";
            value = value + 20;
        } finally {
            value = value + 22;
        }
        marker === "outer" ? value : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn rejects_try_without_catch_or_finally() -> TestResult {
    let Err(error) = eval("try {}") else {
        return Err("expected try without catch or finally to fail".into());
    };
    ensure_error_contains(&error, "expected 'catch' or 'finally'")
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
