use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_function_length_name_and_missing_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let named = function namedCamera(left, right) {
            return left + right;
        };
        let anonymous = [function(one, two, three) {
            return one;
        }][0];

        print(named.length, named.name, named(40, 2));
        print(anonymous.length, anonymous.name === "");
        print("length" in named, "name" in named, "missing" in named, named.missing === undefined);

        named.length === 2 &&
            named.name === "namedCamera" &&
            anonymous.length === 3 &&
            anonymous.name === "" &&
            named.missing === undefined &&
            ("length" in named) === true &&
            ("name" in named) === true &&
            ("missing" in named) === false ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "2 namedCamera 42".to_owned(),
            "3 true".to_owned(),
            "true true false true".to_owned(),
        ],
    )
}

#[test]
fn keeps_builtin_function_properties_non_enumerable() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let fn = function namedCamera(left, right) {
            return left + right;
        };
        let seen = "";
        for (let key in fn) {
            seen = seen + key + ";";
        }
        print(seen, fn.length, fn.name);
        seen === "" && fn.length === 2 && fn.name === "namedCamera" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &[" 2 namedCamera".to_owned()])
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
