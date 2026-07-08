use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_object_static_collection_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let proto = { inherited: 3 };
        let object = Object.create(proto, {
            alpha: { value: 1, enumerable: true, writable: true, configurable: true },
            hidden: { value: 9 }
        });
        object.beta = 2;
        let descriptors = Object.getOwnPropertyDescriptors(object);
        let values = Object.values(object);
        let entries = Object.entries(object);
        let assigned = Object.assign({ seed: 7 }, object, { gamma: 4 }, null, undefined, "xy");

        print(
            Object.create.length,
            Object.assign.length,
            Object.values.length,
            Object.entries.length,
            Object.getOwnPropertyDescriptors.length,
            Object.defineProperties.length
        );
        print(values.length, values[0], values[1]);
        print(entries.length, entries[0][0], entries[0][1], entries[1][0], entries[1][1]);
        print(
            assigned.seed,
            assigned.alpha,
            assigned.beta,
            assigned.gamma,
            assigned[0],
            assigned[1]
        );
        print(
            descriptors.alpha.value,
            descriptors.alpha.enumerable,
            descriptors.hidden.value,
            descriptors.hidden.enumerable,
            "inherited" in object,
            Object.hasOwn(object, "inherited")
        );

        values.length === 2 &&
            values[0] === 1 &&
            values[1] === 2 &&
            entries.length === 2 &&
            entries[0][0] === "alpha" &&
            entries[0][1] === 1 &&
            entries[1][0] === "beta" &&
            entries[1][1] === 2 &&
            assigned.seed === 7 &&
            assigned.alpha === 1 &&
            assigned.beta === 2 &&
            assigned.gamma === 4 &&
            assigned[0] === "x" &&
            assigned[1] === "y" &&
            descriptors.alpha.value === 1 &&
            descriptors.alpha.enumerable === true &&
            descriptors.hidden.value === 9 &&
            descriptors.hidden.enumerable === false &&
            ("inherited" in object) &&
            Object.hasOwn(object, "inherited") === false ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "2 2 1 1 1 2",
            "2 1 2",
            "2 alpha 1 beta 2",
            "7 1 2 4 x y",
            "1 true 9 false true false",
        ],
    )
}

#[test]
fn supports_object_is_and_prototype_mutation() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let root = Object.create(null);
        let left = { name: "left" };
        let right = { name: "right" };
        let child = Object.create(left);
        let returned = Object.setPrototypeOf(child, right);
        let primitive = Object.setPrototypeOf(7, null);

        print(
            Object.is.length,
            Object.setPrototypeOf.length,
            Object.getPrototypeOf(root),
            Object.getPrototypeOf(child) === right
        );
        print(
            Object.is(NaN, NaN),
            Object.is(0, -0),
            Object.is(-0, -0),
            Object.is(child, returned),
            primitive
        );

        Object.getPrototypeOf(root) === null &&
            Object.getPrototypeOf(child) === right &&
            returned === child &&
            primitive === 7 &&
            Object.is(NaN, NaN) === true &&
            Object.is(0, -0) === false &&
            Object.is(-0, -0) === true &&
            Object.is(child, returned) === true ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["2 2 null true", "true false true true 7"],
    )
}

#[test]
fn rejects_define_properties_on_nullish_targets() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let undefined_result = context.eval("Object.defineProperties(undefined, {})");
    ensure_eval_error(&undefined_result)?;
    let null_result = context.eval("Object.defineProperties(null, {})");
    ensure_eval_error(&null_result)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    let actual: Vec<&str> = actual.iter().map(String::as_str).collect();
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_eval_error(result: &rs_quickjs::Result<Value>) -> TestResult {
    if result.is_err() {
        return Ok(());
    }
    Err("expected evaluation to fail".into())
}
