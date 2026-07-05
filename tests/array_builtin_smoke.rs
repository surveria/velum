use rs_quickjs::{Error, Runtime, RuntimeLimits, Value};

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

#[test]
fn preserves_dense_array_element_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [];
        values[2] = "two";
        values[0] = "zero";
        delete values[2];
        values[1] = "one";
        values.extra = 7;

        let seen = "";
        for (let key in values) {
            seen = seen + key + ":" + values[key] + ";";
        }

        print(seen);
        print(values.length, "2" in values, values[2]);

        seen === "0:zero;1:one;extra:7;" &&
            values.length === 3 &&
            !("2" in values) &&
            values[2] === undefined ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "0:zero;1:one;extra:7;".to_owned(),
            "3 false undefined".to_owned(),
        ],
    )
}

#[test]
fn supports_sparse_array_indices_without_dense_growth() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [];
        values.extra = "side";
        values[4097] = "tail";

        let seen = "";
        for (let key in values) {
            seen = seen + key + ":" + values[key] + ";";
        }

        print(values.length, values[4097], "4097" in values);
        print(seen);
        values.length === 4098 &&
            values[4097] === "tail" &&
            ("4097" in values) &&
            seen === "4097:tail;extra:side;" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "4098 tail true".to_owned(),
            "4097:tail;extra:side;".to_owned(),
        ],
    )
}

#[test]
fn counts_dense_array_elements_toward_property_limit() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_object_properties: 1,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();

    let Err(error) = context.eval(
        r"
        let values = [];
        values[0] = 1;
        values[1] = 2;
        ",
    ) else {
        return Err("expected dense array property limit to fail".into());
    };
    ensure_error_contains(&error, "object property count exceeded 1")
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

fn ensure_error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
