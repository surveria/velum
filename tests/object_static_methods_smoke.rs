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

#[test]
fn object_assign_preserves_own_key_order_and_symbol_identity() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let log = "";
        let first = Symbol("first");
        let second = Symbol("second");
        let source = {};
        Object.defineProperty(source, first, {
            enumerable: true,
            get: function() { log = log + ":first"; return 1; }
        });
        Object.defineProperty(source, "alpha", {
            enumerable: true,
            get: function() { log = log + ":alpha"; return 2; }
        });
        Object.defineProperty(source, second, {
            enumerable: true,
            get: function() { log = log + ":second"; return 3; }
        });
        Object.defineProperty(source, "beta", {
            enumerable: true,
            get: function() { log = log + ":beta"; return 4; }
        });
        let target = Object.assign({}, source);
        log === ":alpha:beta:first:second" &&
            target.alpha === 2 && target.beta === 4 &&
            target[first] === 1 && target[second] === 3
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn object_assign_uses_throwing_set_and_array_exotic_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        function throwsTypeError(callback) {
            try {
                callback();
                return false;
            } catch (error) {
                return error instanceof TypeError;
            }
        }

        let readonly = {};
        Object.defineProperty(readonly, "value", { value: 1, writable: false });
        let nonExtensible = Object.preventExtensions({ existing: 1 });
        Object.assign(nonExtensible, { existing: 2 });

        let setterValue = 0;
        let accessor = Object.freeze({
            set value(next) { setterValue = next; }
        });
        Object.assign(accessor, { value: 7 });

        let array = [7, 8, 9];
        Object.assign(array, { 1: 2, length: 2 });
        Object.assign(array, { 3: 4 });

        throwsTypeError(function() { Object.assign(null, {}); }) &&
            throwsTypeError(function() { Object.assign(undefined, {}); }) &&
            throwsTypeError(function() { Object.assign(readonly, { value: 2 }); }) &&
            throwsTypeError(function() { Object.assign(nonExtensible, { added: 3 }); }) &&
            throwsTypeError(function() { Object.assign("a", [1]); }) &&
            nonExtensible.existing === 2 && setterValue === 7 &&
            array.length === 4 && array[0] === 7 && array[1] === 2 &&
            array[2] === undefined && array[3] === 4
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn object_create_uses_catchable_type_errors_and_shared_descriptors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        function throwsTypeError(callback) {
            try {
                callback();
                return false;
            } catch (error) {
                return error instanceof TypeError;
            }
        }

        let prototype = { inherited: 1 };
        let symbol = Symbol("created");
        let properties = {};
        Object.defineProperty(properties, "alpha", {
            enumerable: true,
            get: function() {
                return { value: 2, enumerable: true };
            }
        });
        Object.defineProperty(properties, symbol, {
            enumerable: true,
            value: { value: 3, writable: true }
        });
        let created = Object.create(prototype, properties);
        let argumentsValue = (function() { return arguments; })(1, 2);

        let unchanged = {};
        throwsTypeError(function() {
            Object.defineProperties(unchanged, {
                first: { value: 1 },
                second: null
            });
        });

        throwsTypeError(function() { Object.create(undefined); }) &&
            throwsTypeError(function() { Object.create(true); }) &&
            throwsTypeError(function() { Object.create(2); }) &&
            throwsTypeError(function() { Object.create("prototype"); }) &&
            throwsTypeError(function() { Object.create({}, null); }) &&
            throwsTypeError(function() { Object.create({}, "x"); }) &&
            throwsTypeError(function() { Object.create({}, { value: undefined }); }) &&
            Object.getPrototypeOf(created) === prototype &&
            created.inherited === 1 && created.alpha === 2 && created[symbol] === 3 &&
            Object.prototype.toString.call(argumentsValue) === "[object Arguments]" &&
            Object.prototype.propertyIsEnumerable.call(created, "alpha") &&
            !Object.prototype.propertyIsEnumerable.call(created, symbol) &&
            !Object.hasOwn(unchanged, "first") &&
            Object.getPrototypeOf(Object.create(null, "")) === null
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
