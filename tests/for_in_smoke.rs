use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_for_in_bindings_and_property_order() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { first: 1, second: 2, third: 3 };
        delete object.second;
        object.second = 20;

        let seen = "";
        for (let key in object) {
            seen = seen + key + ":" + object[key] + ";";
        }
        print(seen, typeof key);

        let values = [10, 20];
        values[3] = 40;
        let indexes = "";
        for (const index in values) {
            indexes = indexes + index + "=" + values[index] + ";";
        }
        print(indexes, typeof index);

        var hoisted = "start";
        for (var name in { alpha: 1, beta: 2 }) {
            hoisted = name;
        }
        print(hoisted, typeof name, name);

        seen === "first:1;third:3;second:20;" &&
            indexes === "0=10;1=20;3=40;" &&
            hoisted === "beta" &&
            name === "beta" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "first:1;third:3;second:20; undefined".to_owned(),
            "0=10;1=20;3=40; undefined".to_owned(),
            "beta string beta".to_owned(),
        ],
    )
}

#[test]
fn supports_for_in_assignment_targets_and_control_flow() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let target = { slot: "" };
        let seen = "";
        for (target.slot in { a: 1, b: 2, c: 3 }) {
            if (target.slot === "b") {
                continue;
            }
            seen = seen + target.slot;
            if (target.slot === "c") {
                break;
            }
        }
        print(seen, target.slot);

        let bag = { key: "" };
        let property = "key";
        for (bag[property] in { left: 1, right: 2 }) {
        }
        print(bag.key);

        let pick = function() {
            for (let key in { first: 1, second: 2 }) {
                return key;
            }
            return "none";
        };
        print(pick());

        seen === "ac" && target.slot === "c" && bag.key === "right" && pick() === "first" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["ac c".to_owned(), "right".to_owned(), "first".to_owned()],
    )
}

#[test]
fn creates_fresh_const_binding_per_for_in_iteration() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let first = function() { return "none"; };
        let second = function() { return "none"; };
        let index = 0;

        for (const key in { alpha: 1, beta: 2 }) {
            if (index === 0) {
                first = function() { return key; };
            }
            if (index === 1) {
                second = function() { return key; };
            }
            index = index + 1;
        }

        print(first(), second(), typeof key);
        first() === "alpha" && second() === "beta" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["alpha beta undefined".to_owned()])
}

#[test]
fn rejects_for_in_over_nullish_values() -> TestResult {
    ensure_error_contains("for (let key in null) {}", "Cannot convert")?;
    ensure_error_contains("for (let key in undefined) {}", "Cannot convert")
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

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    error_contains(&error, expected)
}

fn error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
