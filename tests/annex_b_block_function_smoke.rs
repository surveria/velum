use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn sloppy_eval_initializes_and_updates_the_outer_var_binding() -> TestResult {
    expect_true(
        r#"
        var before, inside, after;
        (function() {
            eval("before = f; { function f() { return 7; } inside = f(); } after = f();");
        }());
        before === undefined && inside === 7 && after === 7 && typeof f === "undefined"
        "#,
    )
}

#[test]
fn conditional_block_functions_update_only_when_evaluated() -> TestResult {
    expect_true(
        r#"
        var selected, skipped;
        (function() {
            eval("if (true) function selected() { return 8; }");
            eval("if (false) function skipped() { return 9; }");
            return selected() === 8 && skipped === undefined;
        }())
        "#,
    )
}

#[test]
fn switch_functions_are_initialized_before_case_evaluation() -> TestResult {
    expect_true(
        r#"
        var observed;
        eval("switch (0) { case 0: observed = selected(); break; default: function selected() { return 10; } }");
        observed === 10 && selected === undefined
        "#,
    )
}

#[test]
fn lexical_conflicts_suppress_the_annex_b_var_binding() -> TestResult {
    expect_true(
        r#"
        eval("{ let blocked = 1; { function blocked() {} } }");
        eval("for (let loop = 0; loop < 1; ++loop) { { function loop() {} } }");
        eval("try { throw {}; } catch ({ caught }) { { function caught() {} } }");
        typeof blocked === "undefined" &&
        typeof loop === "undefined" &&
        typeof caught === "undefined"
        "#,
    )
}

#[test]
fn strict_block_functions_do_not_update_outer_vars() -> TestResult {
    expect_true(
        r#"
        "use strict";
        var value = 1;
        { function value() { return 2; } }
        value === 1
        "#,
    )
}

#[test]
fn catch_binding_patterns_destructure_the_thrown_value() -> TestResult {
    expect_true(
        r"
        var first, rest, caught;
        try {
            throw { value: 42, extra: 7 };
        } catch ({ value: first, ...rest }) {
            caught = first === 42 && rest.extra === 7;
        }
        caught && first === undefined && rest === undefined
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
