use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_array_join_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [1, "two", null, undefined, true];
        let defaultJoin = values.join();
        let dashJoin = values.join("-");
        let nullSeparator = [1, 2].join(null);

        let sparse = Array(3);
        sparse[1] = "middle";
        let sparseJoin = sparse.join("|");
        let emptyJoin = [].join();

        let side = 0;
        let marker = function() {
            side = 42;
            return "ignored";
        };
        let extraIgnored = [7].join(undefined, marker());

        Array.prototype[0] = "proto";
        let inherited = Array(2).join("|");
        delete Array.prototype[0];

        let prototypeKeys = "";
        for (let key in Array.prototype) {
            prototypeKeys = prototypeKeys + key + ";";
        }

        print("join", defaultJoin, dashJoin, nullSeparator);
        print("sparse", emptyJoin === "", sparseJoin, extraIgnored, side, inherited);
        print("meta", typeof Array.prototype.join, Array.prototype.join.name, Array.prototype.join.length);
        print("keys:" + prototypeKeys);
        print("in", "join" in values);

        defaultJoin === "1,two,,,true" &&
            dashJoin === "1-two---true" &&
            nullSeparator === "1null2" &&
            emptyJoin === "" &&
            sparseJoin === "|middle|" &&
            extraIgnored === "7" &&
            side === 42 &&
            inherited === "proto|" &&
            prototypeKeys === "" &&
            typeof Array.prototype.join === "function" &&
            Array.prototype.join.name === "join" &&
            Array.prototype.join.length === 1 &&
            ("join" in values) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "join 1,two,,,true 1-two---true 1null2".to_owned(),
            "sparse true |middle| 7 42 proto|".to_owned(),
            "meta function join 1".to_owned(),
            "keys:".to_owned(),
            "in true".to_owned(),
        ],
    )
}

#[test]
fn rejects_array_join_on_non_array_receiver() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval(
        r"
        let object = {};
        object.join = Array.prototype.join;
        object.join('|');
        ",
    ) else {
        return Err("expected Array.prototype.join on non-array receiver to fail".into());
    };
    ensure_error_contains(&error, "requires an array receiver")
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

fn ensure_error_contains(error: &rs_quickjs::Error, text: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(text) {
        return Ok(());
    }

    Err(format!("expected error containing '{text}', got '{message}'").into())
}
