use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn regexp_test_uses_the_receiver_exec_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let calls = 0;
        let receiver = {
            exec(input) {
                calls += 1;
                return input === "camera" ? function () {} : null;
            }
        };
        RegExp.prototype.test.call(receiver, "camera") &&
            !RegExp.prototype.test.call(receiver, "lens") && calls === 2 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn regexp_constructor_copies_internal_slots_without_property_getters() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let source = /camera/gi;
        let getterCalls = 0;
        Object.defineProperty(source, "source", {
            get() { getterCalls += 1; return "lens"; }
        });
        Object.defineProperty(source, "flags", {
            get() { getterCalls += 1; return "m"; }
        });
        let copy = new RegExp(source);
        copy.source === "camera" && copy.flags === "gi" && getterCalls === 0 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
