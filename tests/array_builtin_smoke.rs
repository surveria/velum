use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_array_constructor_and_prototype() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let early = [];
        let arrayConstructor = Array;
        let created = Array();
        let constructed = new Array();
        let withElements = Array("front", 42);
        let withLength = Array(3);
        let originalPrototype = Array.prototype;
        Array.prototype = null;
        let prototypeStayed = Array.prototype === originalPrototype &&
            [].__proto__ === originalPrototype;

        let constructorKeys = "";
        for (let key in Array) {
            constructorKeys = constructorKeys + key + ";";
        }

        let prototypeKeys = "";
        for (let key in Array.prototype) {
            prototypeKeys = prototypeKeys + key + ";";
        }

        print(
            typeof Array,
            Array.name,
            Array.length,
            Array.prototype.constructor === Array
        );
        print(
            early.__proto__ === Array.prototype,
            Array.prototype.__proto__ === Object.prototype,
            early.constructor === Array,
            prototypeStayed
        );
        print(
            created.length,
            constructed.length,
            withElements.length,
            withElements[0],
            withElements[1],
            withLength.length,
            withLength[0]
        );
        print("keys:" + constructorKeys + "|" + prototypeKeys);

        arrayConstructor === Array &&
            Array.prototype.__proto__ === Object.prototype &&
            Array.prototype.constructor.prototype === Array.prototype &&
            early.constructor === Array &&
            created.__proto__ === Array.prototype &&
            constructed.__proto__ === Array.prototype &&
            withElements.length === 2 &&
            withElements[0] === "front" &&
            withElements[1] === 42 &&
            withLength.length === 3 &&
            withLength[0] === undefined &&
            prototypeStayed &&
            constructorKeys === "" &&
            prototypeKeys === "" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "function Array 1 true".to_owned(),
            "true true true true".to_owned(),
            "0 0 2 front 42 3 undefined".to_owned(),
            "keys:|".to_owned(),
        ],
    )
}

#[test]
fn array_intrinsic_does_not_overwrite_user_globals() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let Array = 7;
        let Object = 9;
        let values = [1, 2];
        let constructorName = values.constructor.name;
        print(Array, Object, constructorName, values.constructor === Array);

        Array === 7 &&
            Object === 9 &&
            constructorName === "Array" &&
            values.constructor !== Array ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["7 9 Array false".to_owned()])
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
