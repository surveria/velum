use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_eval_function_metadata() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        typeof eval === "function" &&
            eval.name === "eval" &&
            eval.length === 1 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn eval_executes_string_source() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let value = 40;
        eval("value = value + 2; value");
        value
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_optional_value(context.get_global("value").as_ref(), &Value::Number(42.0))
}

#[test]
fn eval_returns_non_string_inputs() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let object = { value: 42 };
        eval(object) === object &&
            eval(42) === 42 &&
            eval(true) === true &&
            eval(null) === null &&
            eval() === undefined ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_optional_value(actual: Option<&Value>, expected: &Value) -> TestResult {
    let Some(actual) = actual else {
        return Err(format!("expected value {expected:?}, got missing value").into());
    };

    ensure_value(actual, expected)
}
