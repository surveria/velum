use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn supports_object_literal_shorthand_and_methods() -> TestResult {
    let value = eval(
        r#"
        let name = "front-door";
        let count = 40;
        let camera = {
            name,
            count,
            default: 1,
            7: 2,
            add(extra) {
                return this.count + extra;
            },
            nested() {
                return this.add(this[7]);
            },
        };
        ("prototype" in camera.add) ? 0 : camera.nested()
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_missing_shorthand_bindings() -> TestResult {
    let Err(error) = eval("let camera = { missing }; camera.missing") else {
        return Err("expected missing shorthand binding to fail".into());
    };
    let message = error.to_string();
    if message.contains("ReferenceError: 'missing' is not defined") {
        return Ok(());
    }
    Err(format!("expected ReferenceError, got '{message}'").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
