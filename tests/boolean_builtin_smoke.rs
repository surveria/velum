use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_boolean_constructor_and_preserves_shadowing() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let booleanConstructor = Boolean;
        let constructedFalse = new Boolean(false);
        let constructedTrue = new Boolean(1);
        let objectFalse = Object(false);
        let originalPrototype = Boolean.prototype;
        Boolean.prototype = null;
        let prototypeStayed = Boolean.prototype === originalPrototype &&
            (new Boolean()).__proto__ === originalPrototype;

        let constructorKeys = "";
        for (let key in Boolean) {
            constructorKeys = constructorKeys + key + ";";
        }

        let prototypeKeys = "";
        for (let key in Boolean.prototype) {
            prototypeKeys = prototypeKeys + key + ";";
        }

        let shadowResult = 0;
        {
            let Boolean = function(value) {
                return value + 35;
            };
            shadowResult = Boolean(7);
        }

        print(
            typeof Boolean,
            Boolean.name,
            Boolean.length,
            Boolean.prototype.constructor === Boolean
        );
        print(
            Boolean(),
            Boolean(false),
            Boolean(0),
            Boolean(""),
            Boolean(null),
            Boolean(undefined),
            Boolean(true),
            Boolean(1),
            Boolean("camera"),
            Boolean(Object())
        );
        print(
            typeof constructedFalse,
            constructedFalse.__proto__ === Boolean.prototype,
            constructedFalse.constructor === Boolean,
            Boolean(constructedFalse),
            constructedFalse.valueOf(),
            constructedTrue.valueOf(),
            objectFalse.valueOf(),
            objectFalse.toString()
        );
        print("keys:" + constructorKeys + "|" + prototypeKeys);

        booleanConstructor === Boolean &&
            typeof Boolean === "function" &&
            Boolean.name === "Boolean" &&
            Boolean.length === 1 &&
            Boolean.prototype.__proto__ === Object.prototype &&
            Boolean.prototype.constructor.prototype === Boolean.prototype &&
            constructedFalse.__proto__ === Boolean.prototype &&
            constructedTrue.__proto__ === Boolean.prototype &&
            constructedFalse.constructor === Boolean &&
            typeof constructedFalse === "object" &&
            prototypeStayed &&
            constructorKeys === "" &&
            prototypeKeys === "" &&
            shadowResult === 42 &&
            Boolean() === false &&
            Boolean(false) === false &&
            Boolean(0) === false &&
            Boolean("") === false &&
            Boolean(null) === false &&
            Boolean(undefined) === false &&
            Boolean(true) === true &&
            Boolean(1) === true &&
            Boolean("camera") === true &&
            Boolean(Object()) === true &&
            Boolean(constructedFalse) === true &&
            constructedFalse.valueOf() === false &&
            constructedTrue.valueOf() === true &&
            constructedFalse.toString() === "false" &&
            constructedTrue.toString() === "true" &&
            objectFalse.valueOf() === false &&
            objectFalse.toString() === "false" &&
            Boolean.prototype.valueOf.call(true) === true ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "function Boolean 1 true",
            "false false false false false false true true true true",
            "object true true true false true false false",
            "keys:|",
        ],
    )
}

#[test]
fn rejects_boolean_prototype_value_methods_for_wrong_receivers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let error = context.eval("Boolean.prototype.toString.call(0)");
    let Err(error) = error else {
        return Err("expected Boolean.prototype.toString to reject number receiver".into());
    };
    let text = error.to_string();
    if text.contains("Boolean.prototype value method") {
        return Ok(());
    }

    Err(format!("unexpected error: {text}").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
