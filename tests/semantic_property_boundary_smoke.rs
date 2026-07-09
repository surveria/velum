use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn routes_object_like_get_and_has_through_one_boundary() -> TestResult {
    eval_is_42(
        r#"
        let ordinary = { answer: 42 };
        let dynamic = "answer";

        let callable = function BoundaryFunction() {};
        callable.answer = 42;

        Object.answer = 42;
        let error = new TypeError("boundary");
        let boxed = new String("xy");

        ordinary.answer === 42 &&
            ordinary[dynamic] === 42 &&
            dynamic in ordinary &&
            callable.answer === 42 &&
            callable[dynamic] === 42 &&
            dynamic in callable &&
            Object.answer === 42 &&
            Object[dynamic] === 42 &&
            dynamic in Object &&
            error.name === "TypeError" &&
            error.message === "boundary" &&
            "message" in error &&
            "toString" in error &&
            boxed[0] === "x" &&
            "0" in boxed ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_proxy_dispatch_and_explicit_reflect_receiver() -> TestResult {
    eval_is_42(
        r#"
        let receiver = { marker: 42 };
        let getCount = 0;
        let hasCount = 0;
        let proxy = new Proxy({}, {
            get: function (target, key, actualReceiver) {
                getCount += 1;
                if (key === "__proto__") return 42;
                if (key === "value") return actualReceiver.marker;
                if (key === Symbol.toStringTag) return "Boundary";
                return target[key];
            },
            has: function (target, key) {
                hasCount += 1;
                return key === Symbol.toStringTag;
            }
        });

        let directPrototype = proxy.__proto__;
        let reflected = Reflect.get(proxy, "value", receiver);
        let tag = Object.prototype.toString.call(proxy);
        let symbolPresent = Symbol.toStringTag in proxy;

        directPrototype === 42 &&
            reflected === 42 &&
            tag === "[object Boundary]" &&
            symbolPresent === true &&
            getCount === 3 &&
            hasCount === 1 ? 42 : 0
        "#,
    )
}

#[test]
fn routes_symbol_destructuring_descriptors_and_iteration_protocol() -> TestResult {
    eval_is_42(
        r#"
        let tag = Symbol.toStringTag;
        let symbolReads = 0;
        let symbolSource = new Proxy({}, {
            get: function (target, key) {
                if (key === tag) {
                    symbolReads += 1;
                    return 42;
                }
                return target[key];
            }
        });
        let { [tag]: extracted } = symbolSource;
        let fallbackTarget = { [tag]: 42 };
        let fallbackProxy = new Proxy(fallbackTarget, {});
        let fallbackRead = fallbackProxy[tag];
        let fallbackHas = tag in fallbackProxy;

        let descriptorHas = 0;
        let descriptorGet = 0;
        let descriptor = new Proxy({
            value: 42,
            writable: true,
            enumerable: true,
            configurable: true
        }, {
            has: function (target, key) {
                descriptorHas += 1;
                return key in target;
            },
            get: function (target, key) {
                descriptorGet += 1;
                return target[key];
            }
        });
        let defined = {};
        Object.defineProperty(defined, "answer", descriptor);

        let iteratorReads = 0;
        let iterable = {
            [Symbol.iterator]: function () {
                let next = 40;
                return {
                    next: function () {
                        next += 1;
                        return next <= 42
                            ? { value: next, done: false }
                            : { value: undefined, done: true };
                    }
                };
            }
        };
        let proxiedIterable = new Proxy(iterable, {
            get: function (target, key) {
                if (key === Symbol.iterator) iteratorReads += 1;
                return target[key];
            }
        });
        let total = 0;
        for (let value of proxiedIterable) total += value;

        extracted === 42 &&
            symbolReads === 1 &&
            fallbackRead === 42 &&
            fallbackHas === true &&
            defined.answer === 42 &&
            descriptorHas === 6 &&
            descriptorGet === 4 &&
            iteratorReads === 1 &&
            total === 83 ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_current_host_function_property_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_function("hostBoundary", |_call| Ok(Value::Undefined))?;

    let Err(read_error) = context.eval("hostBoundary.missing") else {
        return Err("expected host function property read to fail".into());
    };
    ensure_error_contains(
        &read_error,
        "member access 'missing' is not supported for function",
    )?;

    let Err(has_error) = context.eval("'missing' in hostBoundary") else {
        return Err("expected host function property presence check to fail".into());
    };
    ensure_error_contains(&has_error, "operator 'in' is not supported for function")
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

fn ensure_error_contains(error: &rs_quickjs::Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error containing '{expected}', got '{message}'").into())
}
