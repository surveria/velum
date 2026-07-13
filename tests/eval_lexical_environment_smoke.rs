use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn eval_lexical_declarations_are_isolated_per_call() -> TestResult {
    expect_true(
        r#"
        eval("let value = 1; class C {}; value") === 1 &&
        eval("let value = 2; class C {}; value") === 2 &&
        typeof value === "undefined" &&
        typeof C === "undefined"
        "#,
    )
}

#[test]
fn sloppy_eval_vars_update_the_outer_environment() -> TestResult {
    expect_true(
        r#"
        var value = 1;
        eval("var value = 42; var created = 7");
        value === 42 && created === 7
        "#,
    )
}

#[test]
fn sloppy_eval_extends_shared_parameter_indexes_per_call() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(
        r#"
        function readCamera(lens) {
            eval("var observed = lens + 1");
            return observed;
        }
        readCamera(40) === 41 && readCamera(1) === 2 &&
        typeof observed === "undefined"
        "#,
    )?;
    if value != Value::Bool(true) {
        return Err(format!("expected shared-index copy-on-write result, got {value:?}").into());
    }
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn strict_eval_keeps_vars_local_and_preserves_captured_lexicals() -> TestResult {
    expect_true(
        r#"
        var closure = eval('"use strict"; var hidden = 1; let value = 40; () => ++value');
        closure() === 41 && closure() === 42 &&
        typeof hidden === "undefined" && typeof value === "undefined"
        "#,
    )
}

#[test]
fn declarations_preserve_the_previous_completion_value() -> TestResult {
    expect_true(
        r#"
        eval("1; var first") === 1 &&
        eval("2; let second = 0") === 2 &&
        eval("3; const third = 0") === 3 &&
        eval("4; class Fourth {}") === 4
        "#,
    )
}

#[test]
fn indirect_eval_uses_the_global_variable_environment() -> TestResult {
    expect_true(
        r#"
        function run() {
            let local = 1;
            var indirect = eval;
            indirect("var indirectGlobal = 42; let indirectLexical = 7");
            return local;
        }
        run() === 1 && indirectGlobal === 42 &&
        typeof indirectLexical === "undefined"
        "#,
    )
}

#[test]
fn direct_eval_spread_calls_keep_the_caller_environment() -> TestResult {
    expect_true(
        r#"
        function sloppy() {
            let value = 0;
            eval(...[], "value = 1");
            eval("value = 2", ...[]);
            eval(...["value = 3"]);
            return value;
        }
        function strict() {
            "use strict";
            let value = 0;
            eval(...["value = 4"]);
            return value;
        }
        sloppy() === 3 && strict() === 4
        "#,
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
