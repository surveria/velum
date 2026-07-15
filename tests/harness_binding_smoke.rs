use velum::{Runtime, Value};

mod support;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn print_uses_ordinary_binding_and_call_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function invoke() {
            let print = function (value) {
                return value + 1;
            };
            return print(41);
        }
        typeof print === "function" && print.name === "print" && print.length === 0
            ? invoke()
            : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    if context.output().is_empty() {
        return Ok(());
    }
    Err(format!(
        "shadowed print unexpectedly emitted output: {:?}",
        context.output()
    )
    .into())
}

#[test]
fn assert_harness_requires_the_exact_error_constructor() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    support::install_assert(&mut context)?;

    let value = context.eval(
        r#"
        let rejected = false;
        try {
            assert.throws(Error, function () {
                throw new TypeError("wrong constructor");
            });
        } catch (error) {
            rejected = true;
        }
        rejected ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
