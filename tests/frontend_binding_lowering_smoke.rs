use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn parses_multiline_let_as_a_lexical_declaration() -> TestResult {
    ensure_value(
        &eval(
            r#"
            let
            multilineBinding = 42;
            Object.prototype.hasOwnProperty.call(globalThis, "multilineBinding")
                ? 0
                : multilineBinding
            "#,
        )?,
        &Value::Number(42.0),
    )
}

#[test]
fn materializes_destructuring_for_init_scope() -> TestResult {
    ensure_value(
        &eval(
            r"
            let result = 0;
            for (let [left, right] = [20, 22]; left < 21; left++) {
                result = left + right;
            }
            result
            ",
        )?,
        &Value::Number(42.0),
    )
}

#[test]
fn evaluates_switch_tests_inside_the_switch_lexical_environment() -> TestResult {
    ensure_value(
        &eval(
            r"
            let caught = false;
            try {
                switch (1) {
                    case (switchBinding = 1, 0):
                        break;
                    default:
                        let switchBinding;
                }
            } catch (error) {
                caught = error instanceof ReferenceError;
            }
            caught
            ",
        )?,
        &Value::Bool(true),
    )
}

#[test]
fn strict_delete_throws_and_unqualified_delete_is_an_early_error() -> TestResult {
    ensure_value(
        &eval(
            r#"
            let target = {};
            Object.defineProperty(target, "fixed", { configurable: false });
            let caught = false;
            try {
                (function () {
                    "use strict";
                    delete target.fixed;
                })();
            } catch (error) {
                caught = error instanceof TypeError;
            }
            caught && delete target.fixed === false
            "#,
        )?,
        &Value::Bool(true),
    )?;

    let Err(error) = eval(r#""use strict"; delete unqualified;"#) else {
        return Err("expected strict unqualified delete to fail during parsing".into());
    };
    if matches!(error, Error::Parse { .. }) {
        return Ok(());
    }
    Err(format!("expected parse error, got {error}").into())
}

#[test]
fn strict_for_in_and_for_of_targets_throw_on_failed_writes() -> TestResult {
    ensure_value(
        &eval(
            r#"
            (function () {
                "use strict";
                let target = Object.freeze({ value: 0 });
                let forInCaught = false;
                let forOfCaught = false;
                try {
                    for (target.value in { key: 1 }) {}
                } catch (error) {
                    forInCaught = error instanceof TypeError;
                }
                try {
                    for (target.value of [1]) {}
                } catch (error) {
                    forOfCaught = error instanceof TypeError;
                }
                return forInCaught && forOfCaught;
            })()
            "#,
        )?,
        &Value::Bool(true),
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
