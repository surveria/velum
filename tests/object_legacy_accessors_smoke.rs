use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_legacy_accessor_metadata_and_descriptors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let proto = Object.getOwnPropertyDescriptor(Object.prototype, "__proto__");
        let methods = [
            ["__defineGetter__", 2],
            ["__defineSetter__", 2],
            ["__lookupGetter__", 1],
            ["__lookupSetter__", 1],
        ];
        let methodsValid = methods.every(function (entry) {
            let descriptor = Object.getOwnPropertyDescriptor(Object.prototype, entry[0]);
            return descriptor.value.name === entry[0] &&
                descriptor.value.length === entry[1] &&
                descriptor.writable &&
                !descriptor.enumerable &&
                descriptor.configurable;
        });
        methodsValid &&
            proto.get.name === "get __proto__" &&
            proto.get.length === 0 &&
            proto.set.name === "set __proto__" &&
            proto.set.length === 1 &&
            !proto.enumerable &&
            proto.configurable ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn defines_and_finds_accessors_through_the_prototype_chain() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let symbol = Symbol("value");
        let base = {};
        let stored = 0;
        function getter() { return this.marker; }
        function setter(value) { stored = value; }
        base.__defineGetter__(symbol, getter);
        base.__defineSetter__(symbol, setter);
        let child = { __proto__: base, marker: 40 };
        let descriptor = Object.getOwnPropertyDescriptor(base, symbol);
        let inheritedGetter = child.__lookupGetter__(symbol);
        let inheritedSetter = child.__lookupSetter__(symbol);
        child[symbol] = 2;
        let valueRead = child[symbol];
        Object.defineProperty(child, symbol, { value: 1 });
        let dataStopsLookup = child.__lookupGetter__(symbol) === undefined;

        let callableError = false;
        try { base.__defineGetter__("bad", 1); } catch (error) {
            callableError = error instanceof TypeError;
        }
        descriptor.get === getter &&
            descriptor.set === setter &&
            descriptor.enumerable &&
            descriptor.configurable &&
            inheritedGetter === getter &&
            inheritedSetter === setter &&
            stored === 2 &&
            valueRead === 40 &&
            dataStopsLookup &&
            callableError ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn routes_proto_mutation_and_proxy_failures_through_shared_owners() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let sentinel = {};
        let proxy = new Proxy({}, {
            setPrototypeOf() { throw sentinel; }
        });
        let preserved = false;
        try { proxy.__proto__ = {}; } catch (error) { preserved = error === sentinel; }

        let ordinary = {};
        let parent = { marker: 42 };
        ordinary.__proto__ = parent;
        let assigned = ordinary.marker === 42 && Object.getPrototypeOf(ordinary) === parent;
        ordinary.__proto__ = 1;
        let ignored = Object.getPrototypeOf(ordinary) === parent;

        let immutable = false;
        try { Object.prototype.__proto__ = {}; } catch (error) {
            immutable = error instanceof TypeError;
        }

        let nullRoot = { __proto__: null };
        nullRoot.__proto__ = 7;
        let ownData = Object.getPrototypeOf(nullRoot) === null &&
            Object.hasOwn(nullRoot, "__proto__") &&
            nullRoot.__proto__ === 7;

        preserved && assigned && ignored && immutable && ownData ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn deleted_proto_accessor_leaves_an_ordinary_data_property_name() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const descriptor = Object.getOwnPropertyDescriptor(Object.prototype, "__proto__");
        const target = { marker: 1 };
        const originalPrototype = Object.getPrototypeOf(target);
        const removed = delete Object.prototype.__proto__;

        target.__proto__ = 40;
        target.__proto__ += 1;
        const name = "__proto__";
        target[name] += 1;

        const ordinary = removed &&
            Object.hasOwn(target, name) &&
            target.__proto__ === 42 &&
            Object.getPrototypeOf(target) === originalPrototype;
        Object.defineProperty(Object.prototype, name, descriptor);

        ordinary && target.__proto__ === 42 ? 42 : 0
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
