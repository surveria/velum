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
fn supports_array_is_array_static_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        Array.isArray([]) &&
            Array.isArray(new Array(2)) &&
            Array.isArray(Array.prototype) &&
            Array.prototype.length === 0 &&
            !Array.isArray({ length: 0 }) &&
            !Array.isArray("value") &&
            Array.isArray.length === 1 &&
            Array.isArray.name === "isArray"
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn supports_array_of_constructor_and_property_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function Custom(length) {
            this.constructedLength = length;
        }
        Object.defineProperty(Custom.prototype, "0", {
            set: function() { throw new Error("inherited setter must not run"); }
        });

        let intrinsic = Array.of("first", 2);
        let custom = Array.of.call(Custom, "value");
        let fallback = Array.of.call(Math.abs, 3);

        Array.of.length === 0 &&
            Array.of.name === "of" &&
            Array.isArray(intrinsic) &&
            intrinsic.length === 2 &&
            intrinsic[0] === "first" &&
            intrinsic[1] === 2 &&
            custom instanceof Custom &&
            custom.constructedLength === 1 &&
            custom.length === 1 &&
            custom[0] === "value" &&
            Array.isArray(fallback) &&
            fallback.length === 1 &&
            fallback[0] === 3
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
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
fn preserves_packed_array_semantics_after_hole_transition() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [0, 1, 2, 3];
        delete values[1];
        let before = values.join("|");
        values[1] = "one";
        let after = values.join("|");
        delete values[3];

        let seen = "";
        for (let key in values) {
            seen = seen + key + ":" + values[key] + ";";
        }

        print(before, after, seen, values.length, "1" in values, "3" in values);
        before === "0||2|3" &&
            after === "0|one|2|3" &&
            seen === "0:0;1:one;2:2;" &&
            values.length === 4 &&
            ("1" in values) &&
            !("3" in values) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["0||2|3 0|one|2|3 0:0;1:one;2:2; 4 true false".to_owned()],
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

#[test]
fn counts_packed_array_literals_toward_property_limit() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_object_properties: 1,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();

    let Err(error) = context.eval("[1, 2]") else {
        return Err("expected packed array literal property limit to fail".into());
    };
    ensure_error_contains(&error, "object property count exceeded 1")
}

#[test]
fn preserves_index_access_across_array_methods_and_prototypes() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        Array.prototype[1] = "proto-one";

        let values = ["head"];
        values[2] = "tail";
        let direct = values[1];
        let included = values.includes("proto-one");
        let found = values.indexOf("proto-one");
        let sliced = values.slice(0, 3);
        let joined = values.join("|");

        let shifted = values.shift();
        let afterShift = values.join("|");
        delete Array.prototype[1];

        print(direct, included, found, sliced[1], "1" in sliced);
        print(joined, shifted, afterShift, values.length);

        direct === "proto-one" &&
            included &&
            found === 1 &&
            sliced[1] === "proto-one" &&
            ("1" in sliced) &&
            joined === "head|proto-one|tail" &&
            shifted === "head" &&
            afterShift === "proto-one|tail" &&
            values.length === 2 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "proto-one true 1 proto-one true".to_owned(),
            "head|proto-one|tail head proto-one|tail 2".to_owned(),
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

fn ensure_error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
