use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_number_constructor_and_static_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let numberConstructor = Number;
        let constructed = new Number("7");
        let originalPrototype = Number.prototype;
        Number.prototype = null;
        let prototypeStayed = Number.prototype === originalPrototype &&
            (new Number()).__proto__ === originalPrototype;

        let constructorKeys = "";
        for (let key in Number) {
            constructorKeys = constructorKeys + key + ";";
        }

        let prototypeKeys = "";
        for (let key in Number.prototype) {
            prototypeKeys = prototypeKeys + key + ";";
        }

        let nan = Number.NaN;
        let invalid = Number("front");
        let deleteNan = delete Number.NaN;
        Number.NaN = 7;

        print(
            typeof Number,
            Number.name,
            Number.length,
            Number.prototype.constructor === Number
        );
        print(
            Number(),
            Number(null),
            Number(true),
            Number(false),
            Number(" 42 "),
            Number("1e2"),
            Number("0x10"),
            Number("0b101"),
            Number("0o10")
        );
        print(Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY, Number.NaN);
        print("keys:" + constructorKeys + "|" + prototypeKeys);

        numberConstructor === Number &&
            typeof Number === "function" &&
            Number.name === "Number" &&
            Number.length === 1 &&
            Number.prototype.__proto__ === Object.prototype &&
            Number.prototype.constructor.prototype === Number.prototype &&
            constructed.__proto__ === Number.prototype &&
            constructed.constructor === Number &&
            typeof constructed === "object" &&
            prototypeStayed &&
            constructorKeys === "" &&
            prototypeKeys === "" &&
            Number() === 0 &&
            Number(null) === 0 &&
            Number(true) === 1 &&
            Number(false) === 0 &&
            Number(" 42 ") === 42 &&
            Number("1e2") === 100 &&
            Number("0x10") === 16 &&
            Number("0b101") === 5 &&
            Number("0o10") === 8 &&
            Number("Infinity") === Number.POSITIVE_INFINITY &&
            Number("-Infinity") === Number.NEGATIVE_INFINITY &&
            Number.MAX_VALUE > 1e300 &&
            Number.MIN_VALUE > 0 &&
            Number.EPSILON > 0 &&
            Number.MAX_SAFE_INTEGER === 9007199254740991 &&
            Number.MIN_SAFE_INTEGER === -9007199254740991 &&
            Number.POSITIVE_INFINITY > Number.MAX_VALUE &&
            Number.NEGATIVE_INFINITY < -Number.MAX_VALUE &&
            nan !== nan &&
            invalid !== invalid &&
            Number.NaN !== Number.NaN &&
            deleteNan === false ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "function Number 1 true".to_owned(),
            "0 0 1 0 42 100 16 5 8".to_owned(),
            "Infinity -Infinity NaN".to_owned(),
            "keys:|".to_owned(),
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
