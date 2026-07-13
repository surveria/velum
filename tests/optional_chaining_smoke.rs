use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn static_optional_member_short_circuits_nullish_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let calls = 0;
        function once(value) { calls += 1; return value; }
        [
          once(null)?.value,
          once(undefined)?.value,
          once({ value: 7 })?.value,
          calls
        ].join("|")
        "#,
    )?;
    ensure_value(&value, &Value::from("||7|3"))
}

#[test]
fn question_before_decimal_remains_a_conditional_operator() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval("true?.3:0")?;
    ensure_value(&value, &Value::Number(0.3))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
