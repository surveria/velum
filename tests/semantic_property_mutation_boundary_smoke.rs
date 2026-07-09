use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_symbol_keys_across_proxy_set_and_delete() -> TestResult {
    eval_is_42(
        r#"
        let key = Symbol("mutation");
        let setKey;
        let deleteKey;
        let target = {};
        let proxy = new Proxy(target, {
            set: function (inner, actualKey, value, receiver) {
                setKey = actualKey;
                return Reflect.set(inner, actualKey, value, receiver);
            },
            deleteProperty: function (inner, actualKey) {
                deleteKey = actualKey;
                return Reflect.deleteProperty(inner, actualKey);
            }
        });

        proxy[key] = 42;
        let stored = target[key];
        let deleted = delete proxy[key];

        function callable() {}
        callable.answer = 42;
        let functionWrite = callable.answer;
        let functionDelete = delete callable.answer;

        Object.answer = 42;
        let nativeWrite = Object.answer;
        let nativeDelete = delete Object.answer;

        setKey === key &&
            deleteKey === key &&
            stored === 42 &&
            deleted === true &&
            !(key in target) &&
            functionWrite === 42 &&
            functionDelete === true &&
            nativeWrite === 42 &&
            nativeDelete === true ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_symbol_keys_across_define_descriptor_and_own_keys() -> TestResult {
    eval_is_42(
        r#"
        let symbol = Symbol("descriptor");
        let defineKey;
        let descriptorKey;
        let target = {};
        let proxy = new Proxy(target, {
            defineProperty: function (inner, key, descriptor) {
                defineKey = key;
                return Reflect.defineProperty(inner, key, descriptor);
            },
            ownKeys: function (inner) {
                return Reflect.ownKeys(inner);
            },
            getOwnPropertyDescriptor: function (inner, key) {
                descriptorKey = key;
                return Reflect.getOwnPropertyDescriptor(inner, key);
            }
        });

        Object.defineProperty(proxy, symbol, {
            value: 42,
            enumerable: true,
            writable: true,
            configurable: true
        });
        let descriptor = Object.getOwnPropertyDescriptor(proxy, symbol);
        let keys = Reflect.ownKeys(proxy);
        let symbols = Object.getOwnPropertySymbols(proxy);
        let names = Object.getOwnPropertyNames(proxy);
        let refused = new Proxy({}, {
            defineProperty: function () { return false; }
        });
        let definePropertiesRejected = false;
        try {
            Object.defineProperties(refused, { blocked: { value: 1 } });
        } catch (error) {
            definePropertiesRejected = error instanceof TypeError;
        }
        let reflectDefineRejected =
            Reflect.defineProperty(refused, "blocked", { value: 1 }) === false;

        defineKey === symbol &&
            descriptorKey === symbol &&
            descriptor.value === 42 &&
            keys.length === 1 &&
            keys[0] === symbol &&
            symbols.length === 1 &&
            symbols[0] === symbol &&
            names.length === 0 &&
            definePropertiesRejected === true &&
            reflectDefineRejected === true ? 42 : 0
        "#,
    )
}

#[test]
fn reflect_set_preserves_receiver_across_prototypes_proxies_and_functions() -> TestResult {
    eval_is_42(
        r#"
        let prototype = {};
        Object.defineProperty(prototype, "answer", {
            set: function (value) { this.seen = value; },
            configurable: true
        });
        let target = Object.create(prototype);
        let receiver = {};
        let accessorWrite = Reflect.set(target, "answer", 42, receiver);

        let proxyTarget = {};
        let transparent = new Proxy(proxyTarget, {});
        let proxyReceiver = {};
        let proxyWrite = Reflect.set(transparent, "value", 42, proxyReceiver);

        function callable() {}
        let functionWrite = Reflect.set(callable, "value", 42);

        accessorWrite === true &&
            receiver.seen === 42 &&
            target.seen === undefined &&
            proxyWrite === true &&
            proxyReceiver.value === 42 &&
            proxyTarget.value === undefined &&
            functionWrite === true &&
            callable.value === 42 ? 42 : 0
        "#,
    )
}

#[test]
fn routes_proxy_integrity_and_prototype_operations_through_semantic_methods() -> TestResult {
    eval_is_42(
        r"
        let preventCount = 0;
        let ownKeysCount = 0;
        let descriptorCount = 0;
        let defineCount = 0;
        let target = { answer: 42 };
        let proxy = new Proxy(target, {
            preventExtensions: function (inner) {
                preventCount += 1;
                return Reflect.preventExtensions(inner);
            },
            ownKeys: function (inner) {
                ownKeysCount += 1;
                return Reflect.ownKeys(inner);
            },
            getOwnPropertyDescriptor: function (inner, key) {
                descriptorCount += 1;
                return Reflect.getOwnPropertyDescriptor(inner, key);
            },
            defineProperty: function (inner, key, descriptor) {
                defineCount += 1;
                return Reflect.defineProperty(inner, key, descriptor);
            }
        });

        let frozen = Object.freeze(proxy);
        let isFrozen = Object.isFrozen(proxy);
        proxy.answer = 0;

        let refused = new Proxy({}, {
            setPrototypeOf: function () { return false; }
        });
        let objectRejected = false;
        try {
            Object.setPrototypeOf(refused, null);
        } catch (error) {
            objectRejected = error instanceof TypeError;
        }
        let reflectRejected = Reflect.setPrototypeOf(refused, null) === false;

        frozen === proxy &&
            isFrozen === true &&
            target.answer === 42 &&
            preventCount === 1 &&
            ownKeysCount === 2 &&
            descriptorCount === 2 &&
            defineCount === 1 &&
            objectRejected === true &&
            reflectRejected === true ? 42 : 0
        ",
    )
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected 42, got {value:?}; output: {:?}", context.output()).into())
}
