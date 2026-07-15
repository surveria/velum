use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn iterator_close_uses_break_close_failures() -> TestResult {
    eval_is_42(
        r#"
        function breakWith(iterator) {
            let iterable = {};
            iterable[Symbol.iterator] = function () { return iterator; };
            try {
                for (const value of iterable) { break; }
                return "none";
            } catch (error) {
                return error instanceof TypeError ? "TypeError" : error.message;
            }
        }

        let getterIterator = {
            next: function () { return { done: false, value: 1 }; }
        };
        Object.defineProperty(getterIterator, "return", {
            get: function () { throw new Error("getter"); }
        });
        let callIterator = {
            next: function () { return { done: false, value: 1 }; },
            return: function () { throw new Error("call"); }
        };
        let primitiveIterator = {
            next: function () { return { done: false, value: 1 }; },
            return: function () { return 1; }
        };
        let nonCallableIterator = {
            next: function () { return { done: false, value: 1 }; },
            return: 1
        };

        breakWith(getterIterator) === "getter" &&
            breakWith(callIterator) === "call" &&
            breakWith(primitiveIterator) === "TypeError" &&
            breakWith(nonCallableIterator) === "TypeError" ? 42 : 0
        "#,
    )
}

#[test]
fn iterator_close_preserves_original_throw() -> TestResult {
    eval_is_42(
        r#"
        function throwWith(iterator) {
            let iterable = {};
            iterable[Symbol.iterator] = function () { return iterator; };
            try {
                for (const value of iterable) { throw new Error("body"); }
            } catch (error) {
                return error.message;
            }
        }

        let getterIterator = {
            next: function () { return { done: false, value: 1 }; }
        };
        Object.defineProperty(getterIterator, "return", {
            get: function () { throw new Error("getter"); }
        });
        let callIterator = {
            next: function () { return { done: false, value: 1 }; },
            return: function () { throw new Error("call"); }
        };
        let primitiveIterator = {
            next: function () { return { done: false, value: 1 }; },
            return: function () { return 1; }
        };

        throwWith(getterIterator) === "body" &&
            throwWith(callIterator) === "body" &&
            throwWith(primitiveIterator) === "body" ? 42 : 0
        "#,
    )
}

#[test]
fn iterator_step_validates_results_without_closing() -> TestResult {
    eval_is_42(
        r"
        let closed = false;
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            return {
                next: function () { return 1; },
                return: function () { closed = true; return {}; }
            };
        };
        let rejected = false;
        try {
            for (const value of iterable) {}
        } catch (error) {
            rejected = error instanceof TypeError;
        }
        rejected && closed === false ? 42 : 0
        ",
    )
}

#[test]
fn primitive_strings_honor_an_observable_iterator_method() -> TestResult {
    eval_is_42(
        r"
        String.prototype[Symbol.iterator] = function () {
            let done = false;
            return {
                next: function () {
                    if (done) return { done: true };
                    done = true;
                    return { done: false, value: 42 };
                }
            };
        };
        let result = 0;
        for (const value of 'ignored') { result = value; }
        result
        ",
    )
}

#[test]
fn early_destructuring_uses_iterator_close_validation() -> TestResult {
    eval_is_42(
        r"
        let receiverSeen = false;
        let iterable = {};
        iterable[Symbol.iterator] = function () {
            let iterator = {
                marker: 42,
                next: function () { return { done: false, value: 1 }; },
                return: function () {
                    receiverSeen = this.marker === 42;
                    return 1;
                }
            };
            return iterator;
        };
        let rejected = false;
        try {
            const [first] = iterable;
        } catch (error) {
            rejected = error instanceof TypeError;
        }
        rejected && receiverSeen ? 42 : 0
        ",
    )
}

#[test]
fn native_iterable_consumers_close_after_processing_errors() -> TestResult {
    eval_is_42(
        r#"
        function invalidEntries(log) {
            let iterable = {};
            iterable[Symbol.iterator] = function () {
                return {
                    next: function () { return { done: false, value: 1 }; },
                    return: function () {
                        log.closed = log.closed + 1;
                        throw new Error("close");
                    }
                };
            };
            return iterable;
        }

        let mapLog = { closed: 0 };
        let objectLog = { closed: 0 };
        let mapRejected = false;
        let objectRejected = false;
        try { new Map(invalidEntries(mapLog)); }
        catch (error) { mapRejected = error instanceof TypeError; }
        try { Object.fromEntries(invalidEntries(objectLog)); }
        catch (error) { objectRejected = error instanceof TypeError; }

        mapRejected && objectRejected &&
            mapLog.closed === 1 && objectLog.closed === 1 ? 42 : 0
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
