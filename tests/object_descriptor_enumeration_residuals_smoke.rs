use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn observes_live_enumerability_and_define_semantics() -> TestResult {
    let value = eval(
        r#"
        let source = {
            a: "A",
            get b() {
                delete this.c;
                Object.defineProperty(this, "d", { enumerable: false });
                return "B";
            },
            c: "C",
            d: "D",
        };
        let entries = Object.entries(source);
        let values = Object.values({
            get a() { delete this.b; return 1; },
            b: 2,
        });
        let inheritedSetterCalled = false;
        Object.defineProperty(Object.prototype, "defined", {
            set() { inheritedSetterCalled = true; },
            configurable: true,
        });
        let result = Object.fromEntries([["defined", 42]]);
        entries.length === 2 &&
            entries[0][0] === "a" && entries[0][1] === "A" &&
            entries[1][0] === "b" && entries[1][1] === "B" &&
            values.length === 1 && values[0] === 1 &&
            !inheritedSetterCalled &&
            Object.hasOwn(result, "defined") && result.defined === 42 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn preserves_symbols_string_exotics_and_coercion_order() -> TestResult {
    let value = eval(
        r#"
        let symbol = Symbol("value");
        let object = { plain: 1, [symbol]: 2 };
        let descriptors = Object.getOwnPropertyDescriptors(object);
        let stringDescriptor = Object.getOwnPropertyDescriptor("abc", "1");
        let lengthDescriptor = Object.getOwnPropertyDescriptor(new String("abc"), "length");
        let names = Object.getOwnPropertyNames("ab");
        let keyCalls = 0;
        let key = { toString() { keyCalls = keyCalls + 1; return "x"; } };
        let typeErrors = 0;
        try { Object.defineProperty(null, key, {}); } catch (error) {
            if (error instanceof TypeError) typeErrors = typeErrors + 1;
        }
        try { Object.getOwnPropertyDescriptor(undefined, key); } catch (error) {
            if (error instanceof TypeError) typeErrors = typeErrors + 1;
        }
        try { Object.hasOwn(null, key); } catch (error) {
            if (error instanceof TypeError) typeErrors = typeErrors + 1;
        }
        descriptors.plain.value === 1 && descriptors[symbol].value === 2 &&
            stringDescriptor.value === "b" && !stringDescriptor.writable &&
            stringDescriptor.enumerable && !stringDescriptor.configurable &&
            lengthDescriptor.value === 3 && !lengthDescriptor.writable &&
            !lengthDescriptor.enumerable && !lengthDescriptor.configurable &&
            names.join(",") === "0,1,length" &&
            typeErrors === 3 && keyCalls === 0 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn applies_integrity_to_functions_and_partial_proxy_descriptors() -> TestResult {
    let value = eval(
        r#"
        function ordinary() {}
        ordinary.extra = 1;
        Object.freeze(ordinary);
        let ordinaryDescriptor = Object.getOwnPropertyDescriptor(ordinary, "extra");
        let frozen = Object.isFrozen(ordinary) && !Object.isExtensible(ordinary) &&
            !ordinaryDescriptor.writable && !ordinaryDescriptor.configurable;

        let freezeSeen = {};
        let freezeProxy = new Proxy({ data: 1, get accessor() { return 2; } }, {
            defineProperty(target, key, descriptor) {
                freezeSeen[key] = Object.keys(descriptor).sort().join(",");
                return Reflect.defineProperty(target, key, descriptor);
            },
        });
        Object.freeze(freezeProxy);

        let sealSeen = "";
        let sealProxy = new Proxy({ value: 1 }, {
            defineProperty(target, key, descriptor) {
                sealSeen = Object.keys(descriptor).sort().join(",");
                return Reflect.defineProperty(target, key, descriptor);
            },
        });
        Object.seal(sealProxy);

        frozen &&
            freezeSeen.data === "configurable,writable" &&
            freezeSeen.accessor === "configurable" &&
            sealSeen === "configurable" ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn normalizes_proxy_define_property_descriptors_without_reusing_input() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let attributes = {
            value: 42,
            writable: true,
            enumerable: true,
            configurable: true
        };
        let seen = null;
        let proxy = new Proxy({}, {
            defineProperty(target, key, descriptor) {
                seen = descriptor;
                return Reflect.defineProperty(target, key, descriptor);
            }
        });

        let result = Reflect.defineProperty(proxy, "answer", attributes);
        result === true && proxy.answer === 42 && seen !== attributes &&
            seen.value === 42 && seen.writable === true &&
            seen.enumerable === true && seen.configurable === true ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn reads_property_descriptor_fields_in_spec_order() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let order = "";
        let attributes = {};
        for (let key of [
            "enumerable", "configurable", "value", "writable", "get", "set"
        ]) {
            Object.defineProperty(attributes, key, {
                get() {
                    order = order + key + ",";
                    if (key === "value") return 42;
                    if (key === "get" || key === "set") return undefined;
                    return false;
                }
            });
        }

        let typeError = false;
        try {
            Object.defineProperty({}, "answer", attributes);
        } catch (error) {
            typeError = error instanceof TypeError;
        }
        typeError && order === "enumerable,configurable,value,writable,get,set," ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn routes_bound_function_writes_through_function_prototype_accessors() -> TestResult {
    let value = eval(
        r#"
        function source() {}
        let stored = 1;
        Object.defineProperty(Function.prototype, "shared", {
            get() { return stored; },
            set(value) { stored = value; },
            configurable: true,
        });
        let bound = source.bind(null);
        bound.shared = 42;
        !Object.hasOwn(bound, "shared") && bound.shared === 42 && stored === 42 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

fn eval(source: &str) -> velum::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
