use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn keeps_function_prototype_constructor_non_enumerable() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let Camera = function Camera() {};
        let Replacement = function Replacement() {};

        let beforeKeys = "";
        for (let key in Camera.prototype) {
            beforeKeys = beforeKeys + key + ";";
        }
        let beforeHas = "constructor" in Camera.prototype;
        let beforeSame = Camera.prototype.constructor === Camera;

        Camera.prototype.constructor = Replacement;
        let afterSetKeys = "";
        for (let key in Camera.prototype) {
            afterSetKeys = afterSetKeys + key + ";";
        }
        let afterSetSame = Camera.prototype.constructor === Replacement;

        let deleted = delete Camera.prototype.constructor;

        Camera.prototype.constructor = Camera;
        let afterReaddKeys = "";
        for (let key in Camera.prototype) {
            afterReaddKeys = afterReaddKeys + key + ";";
        }

        print("keys:" + beforeKeys + "|" + afterSetKeys + "|" + afterReaddKeys);
        print(beforeHas, beforeSame, afterSetSame, deleted);

        beforeKeys === "" &&
            beforeHas &&
            beforeSame &&
            afterSetKeys === "" &&
            afterSetSame &&
            deleted &&
            afterReaddKeys === "constructor;" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "keys:||constructor;".to_owned(),
            "true true true true".to_owned(),
        ],
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
