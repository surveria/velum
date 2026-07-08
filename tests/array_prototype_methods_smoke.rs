use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_array_push_and_pop_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [1];
        let firstPush = values.push(2, 3);
        let secondPush = values.push();
        let popped = values.pop("ignored");
        let afterPopLength = values.length;
        delete values[1];
        let hole = values.pop();
        let last = values.pop();
        let empty = values.pop();

        let side = 0;
        let marker = function() {
            side = 42;
            return "ignored";
        };
        [7].pop(marker());

        let prototypeKeys = "";
        for (let key in Array.prototype) {
            prototypeKeys = prototypeKeys + key + ";";
        }

        let arrayKeys = "";
        for (let key in [4, 5]) {
            arrayKeys = arrayKeys + key + ";";
        }

        print(
            "methods",
            typeof Array.prototype.push,
            Array.prototype.push.name,
            Array.prototype.push.length,
            typeof Array.prototype.pop,
            Array.prototype.pop.name,
            Array.prototype.pop.length
        );
        print(
            "values",
            firstPush,
            secondPush,
            popped,
            afterPopLength,
            hole,
            last,
            empty,
            values.length,
            side
        );
        print("keys:" + prototypeKeys + "|" + arrayKeys);
        print("in", "push" in values, "pop" in values);

        firstPush === 3 &&
            secondPush === 3 &&
            popped === 3 &&
            afterPopLength === 2 &&
            hole === undefined &&
            last === 1 &&
            empty === undefined &&
            values.length === 0 &&
            side === 42 &&
            prototypeKeys === "" &&
            arrayKeys === "0;1;" &&
            ("push" in values) &&
            ("pop" in values) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "methods function push 1 function pop 0".to_owned(),
            "values 3 3 3 2 undefined 1 undefined 0 42".to_owned(),
            "keys:|0;1;".to_owned(),
            "in true true".to_owned(),
        ],
    )
}

#[test]
fn supports_push_and_pop_on_array_like_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { length: 1, 0: "head" };
        let pushed = Array.prototype.push.call(object, "tail", undefined);
        let popped = Array.prototype.pop.call(object);
        pushed === 3 &&
            popped === undefined &&
            object.length === 2 &&
            object[0] === "head" &&
            object[1] === "tail" &&
            !("2" in object) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn keeps_non_configurable_pop_on_generic_path() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [1, 2];
        Object.defineProperty(values, "1", {
            value: 2,
            configurable: false,
            enumerable: true,
            writable: true
        });
        let popped = values.pop();
        print("descriptor", popped, values.length, values[1], "1" in values);
        popped === 2 &&
            values.length === 1 &&
            values[1] === 2 &&
            ("1" in values) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["descriptor 2 1 2 true".to_owned()])
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
