use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_custom_function_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let fn = function namedCamera(left, right) {
            return left + right;
        };
        fn.alpha = 1;
        fn["beta"] = 2;
        fn.alpha += 40;
        fn.count = fn(20, 22);

        print(fn.alpha, fn.beta, fn.count, fn.length, fn.name);
        print("alpha" in fn, "beta" in fn, "length" in fn, "missing" in fn);

        fn.alpha === 41 &&
            fn.beta === 2 &&
            fn.count === 42 &&
            fn.length === 2 &&
            fn.name === "namedCamera" &&
            ("alpha" in fn) === true &&
            ("beta" in fn) === true &&
            ("length" in fn) === true &&
            ("missing" in fn) === false ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "41 2 42 2 namedCamera".to_owned(),
            "true true true false".to_owned(),
        ],
    )
}

#[test]
fn enumerates_and_deletes_custom_function_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let fn = function namedCamera() {
            return 42;
        };
        fn.first = 1;
        fn.second = 2;
        delete fn.first;
        fn.third = 3;
        fn.first = 10;

        let seen = "";
        for (let key in fn) {
            seen = seen + key + ":" + fn[key] + ";";
        }
        print(seen);

        delete fn.second;
        print("second" in fn, fn.second === undefined);

        seen === "second:2;third:3;first:10;" &&
            ("second" in fn) === false &&
            fn.second === undefined ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "second:2;third:3;first:10;".to_owned(),
            "false true".to_owned(),
        ],
    )
}

#[test]
fn preserves_out_of_order_function_property_lookup_with_vector_index() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let fn = function namedCamera() {
            return 42;
        };
        fn.zeta = 1;
        fn.alpha = 2;
        fn.middle = 3;
        fn.zeta = fn.zeta + fn.alpha;
        Object.defineProperty(fn, "middle", {
            value: fn.middle + fn.zeta,
            enumerable: true,
            writable: true,
            configurable: true
        });
        let alphaDescriptor = Object.getOwnPropertyDescriptor(fn, "alpha");
        let deleteAlpha = delete fn.alpha;
        fn.alpha = 20;

        let seen = "";
        for (let key in fn) {
            seen = seen + key + ":" + fn[key] + ";";
        }
        print(seen);
        print(alphaDescriptor.value, deleteAlpha, "middle" in fn);

        seen === "zeta:3;middle:6;alpha:20;" &&
            alphaDescriptor.value === 2 &&
            deleteAlpha === true &&
            ("zeta" in fn) &&
            ("alpha" in fn) &&
            ("middle" in fn) &&
            fn.zeta === 3 &&
            fn.alpha === 20 &&
            fn.middle === 6 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "zeta:3;middle:6;alpha:20;".to_owned(),
            "2 true true".to_owned(),
        ],
    )
}

#[test]
fn keeps_builtin_function_metadata_read_only() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let fn = function namedCamera(left, right) {
            return left + right;
        };
        fn.length = 99;
        fn.name = "changed";
        print(fn.length, fn.name);

        let seen = "";
        for (let key in fn) {
            seen = seen + key + ";";
        }
        print(seen);

        fn.length === 2 && fn.name === "namedCamera" && seen === "" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["2 namedCamera".to_owned(), String::new()],
    )
}

#[test]
fn supports_accessor_descriptors_on_javascript_functions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let stored = 0;
        function sample() {}
        Object.defineProperty(sample, "value", {
            get() { return stored; },
            set(next) { stored = next; },
            configurable: true
        });
        Object.defineProperty(sample, "name", {
            get() { return "renamed"; },
            configurable: true
        });
        sample.value = 42;
        const descriptor = Object.getOwnPropertyDescriptor(sample, "value");
        sample.value === 42 &&
            sample.name === "renamed" &&
            typeof descriptor.get === "function" &&
            typeof descriptor.set === "function" &&
            descriptor.enumerable === false &&
            descriptor.configurable === true ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn javascript_functions_use_inherited_restricted_accessors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function* values() {}
        let readThrew = false;
        let writeThrew = false;
        try {
            returnValue = values.caller;
        } catch (error) {
            readThrew = error instanceof TypeError;
        }
        try {
            values.arguments = 42;
        } catch (error) {
            writeThrew = error instanceof TypeError;
        }
        readThrew &&
            writeThrew &&
            !values.hasOwnProperty("caller") &&
            ("caller" in values)
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
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
