use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn expect_number(source: &str, expected: f64) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if actual == Value::Number(expected) {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual:?}").into())
}

#[test]
fn named_function_expression_recurses_without_leaking_its_name() -> TestResult {
    expect_number(
        r#"
        let outside = 7;
        let factorial = function inside(value) {
            return value === 0 ? 1 : value * inside(value - 1);
        };
        let score = factorial(5);
        if (typeof inside === "undefined") score = score + 1;
        if (outside === 7) score = score + 1;
        score
        "#,
        122.0,
    )
}

#[test]
fn parameters_and_defaults_shadow_or_observe_the_private_name() -> TestResult {
    expect_number(
        r"
        let parameterShadow = function self(self) {
            return self;
        };
        let defaultObserver = function self(value = self) {
            return function() { return value === self; };
        };
        let score = parameterShadow(40);
        defaultObserver()() ? score + 2 : 0
        ",
        42.0,
    )
}

#[test]
fn private_name_writes_follow_sloppy_and_strict_modes() -> TestResult {
    expect_number(
        r#"
        let sloppy = function self() {
            self = 1;
            self += 1;
            return typeof self === "function" ? 2 : 0;
        };
        let strict = function self() {
            "use strict";
            let caught = 0;
            try { self = 1; } catch (error) {
                if (error instanceof TypeError) caught = caught + 1;
            }
            try { self += 1; } catch (error) {
                if (error instanceof TypeError) caught = caught + 1;
            }
            return caught;
        };
        38 + sloppy() + strict()
        "#,
        42.0,
    )
}

#[test]
fn direct_eval_inherits_the_callers_strict_mode() -> TestResult {
    expect_number(
        r#"
        let sloppy = function self() {
            eval("self = 1");
            return typeof self === "function" ? 1 : 0;
        };
        let strict = function self() {
            "use strict";
            try {
                eval("self = 1");
            } catch (error) {
                return error instanceof TypeError ? 1 : 0;
            }
            return 0;
        };
        40 + sloppy() + strict()
        "#,
        42.0,
    )
}
