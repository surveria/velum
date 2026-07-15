use velum::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn with_resolves_reads_writes_calls_and_escaping_closures() -> TestResult {
    expect_true(
        r#"
        var outer = 1;
        var escaped;
        var environment = {
            outer: 2,
            method: function() { return this.outer; }
        };
        with (environment) {
            outer++;
            escaped = function() { return outer; };
            if (method() !== 3) throw new Error("wrong with call receiver");
        }
        if (escaped() !== 3) throw new Error("closure lost with environment");
        environment[Symbol.unscopables] = { outer: true };
        escaped() === 1 && environment.outer === 3
        "#,
    )
}

#[test]
fn nested_lexical_bindings_and_inner_with_objects_keep_spec_order() -> TestResult {
    expect_true(
        r"
        var x = 0;
        var first = { x: 1 };
        var second = { x: 2 };
        var lexical;
        var dynamic;
        with (first) {
            { let x = 7; lexical = x; }
            with (second) { dynamic = x; }
        }
        lexical === 7 && dynamic === 2
        ",
    )
}

#[test]
fn assignment_keeps_the_with_reference_across_rhs_side_effects() -> TestResult {
    expect_true(
        r#"
        var typed = new Int32Array(2);
        var environment = Object.create(typed);
        Object.defineProperty(environment, "NaN", {
            configurable: true,
            value: 100
        });
        with (environment) {
            NaN = (delete environment.NaN, 0);
        }
        Object.getOwnPropertyDescriptor(environment, "NaN") === undefined
        "#,
    )
}

#[test]
fn with_updates_empty_abrupt_completion_values() -> TestResult {
    expect_true(
        r#"
        eval("1; do { 2; with ({}) { 3; break; } } while (false)") === 3 &&
        eval("4; do { 5; with ({}) { break; } } while (false)") === undefined &&
        eval("6; do { 7; with ({}) { 8; continue; } } while (false)") === 8 &&
        eval("9; do { 10; with ({}) { continue; } } while (false)") === undefined
        "#,
    )
}

#[test]
fn strict_and_declaration_statement_forms_are_early_errors() -> TestResult {
    for source in [
        r#""use strict"; with ({}) {}"#,
        "with ({}) function f() {}",
        "with ({}) async function f() {}",
        "with ({}) class C {}",
        "with ({}) label: function f() {}",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        let Err(error) = context.eval(source) else {
            return Err(format!("expected parse failure for {source:?}").into());
        };
        if !matches!(error, Error::Parse { .. }) {
            return Err(format!("expected parse error for {source:?}, got {error:?}").into());
        }
    }
    Ok(())
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
