use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_object_constructor_and_prototype() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let plain = {};
        let objectConstructor = Object;
        let created = Object();
        let constructed = new Object();
        let returned = Object(plain);
        let returnedFromNew = new Object(plain);
        let originalPrototype = Object.prototype;
        Object.prototype = null;
        let prototypeStayed = Object.prototype === originalPrototype &&
            (new Object()).__proto__ === originalPrototype;

        let constructorKeys = "";
        for (let key in Object) {
            constructorKeys = constructorKeys + key + ";";
        }

        let prototypeKeys = "";
        for (let key in Object.prototype) {
            prototypeKeys = prototypeKeys + key + ";";
        }

        print(
            typeof Object,
            Object.name,
            Object.length,
            Object.prototype.constructor === Object
        );
        print(
            created.__proto__ === Object.prototype,
            constructed.__proto__ === Object.prototype,
            returned === plain,
            returnedFromNew === plain,
            prototypeStayed
        );
        print("keys:" + constructorKeys + "|" + prototypeKeys);

        objectConstructor === Object &&
            Object.prototype.__proto__ === null &&
            Object.prototype.constructor.prototype === Object.prototype &&
            plain.constructor === Object &&
            created.__proto__ === Object.prototype &&
            constructed.__proto__ === Object.prototype &&
            returned === plain &&
            returnedFromNew === plain &&
            prototypeStayed &&
            constructorKeys === "" &&
            prototypeKeys === "" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "function Object 1 true".to_owned(),
            "true true true true true".to_owned(),
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
