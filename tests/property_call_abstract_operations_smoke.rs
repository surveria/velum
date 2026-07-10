use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn get_method_preserves_lookup_and_call_receivers() -> TestResult {
    eval_is_42(
        r#"
        let events = "";
        let target = { answer: 40 };
        let handler = {
            get get() {
                events = events + "g";
                return function (innerTarget, key, receiver) {
                    events = events + (this === handler ? "c" : "x");
                    return Reflect.get(innerTarget, key, receiver) + 2;
                };
            }
        };
        let proxied = new Proxy(target, handler);
        let fallback = new Proxy({ answer: 42 }, { get: null });
        proxied.answer === 42 && fallback.answer === 42 && events === "gc" ? 42 : 0
        "#,
    )
}

#[test]
fn get_method_rejects_non_callable_values_and_propagates_getters() -> TestResult {
    eval_is_42(
        r#"
        let score = 40;
        let nonCallable = new Proxy({}, { get: 1 });
        try {
            nonCallable.value;
        } catch (error) {
            if (error instanceof TypeError) score = score + 1;
        }

        let handler = {};
        Object.defineProperty(handler, "get", {
            get: function () { throw new TypeError("trap lookup"); }
        });
        try {
            new Proxy({}, handler).value;
        } catch (error) {
            if (error instanceof TypeError) score = score + 1;
        }
        score
        "#,
    )
}

#[test]
fn method_operations_cover_symbol_hooks_and_invoke() -> TestResult {
    eval_is_42(
        r#"
        let primitiveReceiver = false;
        let value = {};
        Object.defineProperty(value, Symbol.toPrimitive, {
            get: function () {
                return function (hint) {
                    primitiveReceiver = this === value && hint === "number";
                    return 40;
                };
            }
        });

        let instanceReceiver = false;
        let matcher = {};
        matcher[Symbol.hasInstance] = function (candidate) {
            instanceReceiver = this === matcher;
            return candidate === 42;
        };

        let localeReceiver = false;
        let locale = {
            toString: function () {
                localeReceiver = this === locale;
                return "42";
            }
        };

        let rejected = false;
        let invalid = { valueOf: function () { return 1; } };
        invalid[Symbol.toPrimitive] = 1;
        try {
            Number(invalid);
        } catch (error) {
            rejected = error instanceof TypeError;
        }

        Number(value) === 40 &&
            (42 instanceof matcher) &&
            Object.prototype.toLocaleString.call(locale) === "42" &&
            primitiveReceiver && instanceReceiver && localeReceiver && rejected ? 42 : 0
        "#,
    )
}

#[test]
fn set_preserves_receiver_and_failure_behavior() -> TestResult {
    eval_is_42(
        r#"
        let target = {};
        let receiver = {};
        let reflected = Reflect.set(target, "answer", 42, receiver);

        let fixed = {};
        Object.defineProperty(fixed, "answer", {
            value: 1,
            writable: false,
            configurable: true
        });
        let rejected = Reflect.set(fixed, "answer", 42) === false && fixed.answer === 1;

        let regexpRejected = false;
        let regexp = /a/g;
        Object.defineProperty(regexp, "lastIndex", { writable: false });
        try {
            regexp.exec("a");
        } catch (error) {
            regexpRejected = error instanceof TypeError;
        }

        reflected && target.answer === undefined && receiver.answer === 42 &&
            rejected && regexpRejected ? 42 : 0
        "#,
    )
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected value 42, got {value:?}").into())
}
