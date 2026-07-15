use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_function_length_name_and_missing_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let named = function namedCamera(left, right) {
            return left + right;
        };
        let anonymous = [function(one, two, three) {
            return one;
        }][0];

        print(named.length, named.name, named(40, 2));
        print(anonymous.length, anonymous.name === "");
        print("length" in named, "name" in named, "missing" in named, named.missing === undefined);

        named.length === 2 &&
            named.name === "namedCamera" &&
            anonymous.length === 3 &&
            anonymous.name === "" &&
            named.missing === undefined &&
            ("length" in named) === true &&
            ("name" in named) === true &&
            ("missing" in named) === false ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "2 namedCamera 42".to_owned(),
            "3 true".to_owned(),
            "true true false true".to_owned(),
        ],
    )
}

#[test]
fn keeps_builtin_function_properties_non_enumerable() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let fn = function namedCamera(left, right) {
            return left + right;
        };
        let seen = "";
        for (let key in fn) {
            seen = seen + key + ";";
        }
        print(seen, fn.length, fn.name);
        seen === "" && fn.length === 2 && fn.name === "namedCamera" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &[" 2 namedCamera".to_owned()])
}

#[test]
fn exposes_and_restores_each_legacy_activation_arguments_object() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function probe(value, extra) {
            let terminal = value === 0;
            let active = probe.arguments;
            active[0] = value + 1;
            if (terminal) {
                return active[0] + ":" + active.length + ":" + (active[1] === undefined);
            }
            let nested = probe(0, "nested");
            return active[0] + ":" + nested + ":" + active[1];
        }

        let before = probe.arguments;
        let result = probe(4, "outer");
        let after = probe.arguments;
        before === null && after === null && result === "5:1:2:false:outer" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn reuses_the_implicit_arguments_object_for_legacy_introspection() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function probe(value) {
            let same = probe.arguments === arguments;
            probe.arguments[0] = 42;
            return same && value === 42 && arguments[0] === 42;
        }
        probe(1) && probe.arguments === null ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn restricts_caller_and_arguments_on_every_non_legacy_function() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let container = {
            method() {},
            get getter() { return 1; },
            set setter(value) {},
        };
        let descriptor = Object.getOwnPropertyDescriptor(container, "getter");
        let setterDescriptor = Object.getOwnPropertyDescriptor(container, "setter");
        let functions = [
            container.method,
            descriptor.get,
            setterDescriptor.set,
            () => {},
            async function() {},
            function*() {},
            class Example {},
        ];
        let restricted = 0;
        for (let fn of functions) {
            for (let property of ["arguments", "caller"]) {
                try {
                    fn[property];
                } catch (error) {
                    if (error instanceof TypeError) restricted = restricted + 1;
                }
            }
        }
        restricted === functions.length * 2 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
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
