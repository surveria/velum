use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn script_await_identifier_remains_a_simple_assignment_target() -> TestResult {
    expect_true(
        r"
        var await = 1;
        await = 2;
        await += 3;
        await++;
        ++await;
        await === 7;
        ",
    )?;
    expect_true(
        r#"
        "use strict";
        var await = 1;
        await = 4;
        await === 4;
        "#,
    )
}

#[test]
fn script_await_identifier_can_be_called_and_indexed() -> TestResult {
    expect_true(
        r"
        function await(value) { return value; }
        var holder = { await: await };
        await(20) + [holder.await][0](22) === 42;
        ",
    )
}

#[test]
fn sloppy_call_assignment_targets_throw_before_rhs_and_conversion() -> TestResult {
    expect_true(
        r"
        var calls = 0;
        var conversions = 0;
        var rhsCalls = 0;
        function target() {
            calls += 1;
            return { valueOf: function () { conversions += 1; return 1; } };
        }
        function rhs() { rhsCalls += 1; return 1; }
        var caught = 0;
        try { target() = rhs(); } catch (error) { caught += error instanceof ReferenceError; }
        try { target() += rhs(); } catch (error) { caught += error instanceof ReferenceError; }
        try { target()++; } catch (error) { caught += error instanceof ReferenceError; }
        try { ++target(); } catch (error) { caught += error instanceof ReferenceError; }
        caught === 4 && calls === 4 && conversions === 0 && rhsCalls === 0;
        ",
    )
}

#[test]
fn sloppy_call_loop_targets_throw_after_call_and_strict_code_is_rejected() -> TestResult {
    expect_true(
        r"
        var calls = 0;
        function target() { calls += 1; return {}; }
        var caught = 0;
        try { for (target() in { key: 1 }) {} } catch (error) {
            caught += error instanceof ReferenceError;
        }
        try { for (target() of [1]) {} } catch (error) {
            caught += error instanceof ReferenceError;
        }
        caught === 2 && calls === 2;
        ",
    )?;

    let runtime = Runtime::new();
    let mut context = runtime.context();
    let error = context
        .eval(r#""use strict"; function target() {} target() = 1;"#)
        .err()
        .ok_or("expected strict call assignment target to fail parsing")?;
    if error.to_string().contains("parser error") {
        return Ok(());
    }
    Err(format!("expected parser error, got {error}").into())
}

#[test]
fn destructuring_lookahead_balances_mixed_nested_delimiters() -> TestResult {
    expect_true(
        r"
        let first;
        let second;
        ({ outer: [first, { value: second }] } = {
            outer: [20, { value: 22 }]
        });
        first + second === 42;
        ",
    )
}

fn expect_true(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}
