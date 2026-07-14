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
fn optional_member_calls_preserve_receivers_and_skip_nullish_arguments() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let calls = 0;
        function argument() { calls += 1; return 1; }
        const object = {
          value: 41,
          add(value) { return this.value + value; }
        };
        const direct = object?.add(1);
        const spread = object?.add(...[1]);
        const skipped = null?.add(argument());
        [direct, spread, skipped, calls].join("|")
        "#,
    )?;
    ensure_value(&value, &Value::from("42|42||0"))
}

#[test]
fn question_before_decimal_remains_a_conditional_operator() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval("true?.3:0")?;
    ensure_value(&value, &Value::Number(0.3))
}

#[test]
fn optional_chains_cannot_tag_templates() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let result = context.eval("const target = { tag() {} }; target?.tag`value`;");
    if result.is_err() {
        return Ok(());
    }
    Err("expected optional-chain tagged template to fail during parsing".into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
