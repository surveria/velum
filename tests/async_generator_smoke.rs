use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

#[test]
fn async_generator_yields_and_returns_through_promises() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        async function* values() {
            yield 40;
            return 42;
        }
        const iterator = values();
        iterator.next().then(function(result) {
            trace = trace + result.value + ":" + result.done;
        });
        iterator.next().then(function(result) {
            trace = trace + ":" + result.value + ":" + result.done;
        });
        "#,
    )?;
    let value = context.eval("trace")?;
    ensure_value(&value, &Value::from("40:false:42:true"))
}

#[test]
fn async_generator_resumes_after_await_and_preserves_request_order() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        async function* values() {
            const value = await Promise.resolve(40);
            yield value;
            return 42;
        }
        const iterator = values();
        iterator.next().then(function(result) {
            trace = trace + result.value + ":" + result.done;
        });
        iterator.next().then(function(result) {
            trace = trace + ":" + result.value + ":" + result.done;
        });
        "#,
    )?;
    let value = context.eval("trace")?;
    ensure_value(&value, &Value::from("40:false:42:true"))
}

#[test]
fn async_generator_exposes_distinct_protocol_prototypes() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        async function* values() {}
        const functionPrototype = Object.getPrototypeOf(values);
        const generatorPrototype = functionPrototype.prototype;
        const iterator = values();
        Object.getPrototypeOf(values.prototype) === generatorPrototype &&
            Object.getPrototypeOf(iterator) === values.prototype &&
            typeof generatorPrototype.next === "function" &&
            generatorPrototype.next.length === 1 &&
            generatorPrototype[Symbol.toStringTag] === "AsyncGenerator" &&
            Object.getPrototypeOf(generatorPrototype)[Symbol.asyncIterator]() ===
                Object.getPrototypeOf(generatorPrototype)
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn async_generator_awaits_yielded_and_returned_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        async function* values() {
            yield Promise.resolve(40);
            return Promise.resolve(42);
        }
        const iterator = values();
        iterator.next().then(function(result) {
            trace = trace + result.value + ":" + result.done;
        });
        iterator.next().then(function(result) {
            trace = trace + ":" + result.value + ":" + result.done;
        });
        "#,
    )?;
    let value = context.eval("trace")?;
    ensure_value(&value, &Value::from("40:false:42:true"))
}

#[test]
fn rejected_yield_value_reenters_generator_control_flow() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r"
        let value = 0;
        async function* values() {
            try {
                yield Promise.reject(40);
            } catch (error) {
                return error + 2;
            }
        }
        values().next().then(function(result) {
            value = result.value;
        });
        ",
    )?;
    let value = context.eval("value")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn exposes_async_generator_function_constructor() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        const AsyncGeneratorFunction = Object.getPrototypeOf(async function* () {}).constructor;
        const generated = AsyncGeneratorFunction("left", "right", "yield left + right;");
        generated(20, 22).next().then(function(result) {
            trace = AsyncGeneratorFunction.name + ":" +
                AsyncGeneratorFunction.length + ":" + result.value + ":" + result.done;
        });
        "#,
    )?;
    let value = context.eval("trace")?;
    ensure_value(&value, &Value::from("AsyncGeneratorFunction:1:42:false"))
}

#[test]
fn async_generator_awaits_generic_thenables() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        const thenable = {
            then(resolve) {
                resolve(40);
            }
        };
        async function* values() {
            yield await thenable;
            return {
                then(resolve) {
                    resolve(42);
                }
            };
        }
        const iterator = values();
        iterator.next().then(function(result) {
            trace = trace + result.value;
        });
        iterator.next().then(function(result) {
            trace = trace + ":" + result.value;
        });
        "#,
    )?;
    let value = context.eval("trace")?;
    ensure_value(&value, &Value::from("40:42"))
}

#[test]
fn async_generator_delegates_to_async_and_sync_iterables() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        async function* inner() {
            yield Promise.resolve(20);
            return 22;
        }
        async function* outer() {
            const delegated = yield* inner();
            yield* [delegated, 42];
        }
        const iterator = outer();
        iterator.next().then(function(result) {
            trace = trace + result.value;
        });
        iterator.next().then(function(result) {
            trace = trace + ":" + result.value;
        });
        iterator.next().then(function(result) {
            trace = trace + ":" + result.value;
        });
        "#,
    )?;
    let value = context.eval("trace")?;
    ensure_value(&value, &Value::from("20:22:42"))
}

#[test]
fn async_generator_awaits_values_from_a_delegated_sync_iterator() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let resolveDelegated;
        let trace = "pending";
        const delayed = new Promise(function(resolve) {
            resolveDelegated = resolve;
        });
        async function* values() {
            yield* [delayed];
        }
        values().next().then(function(result) {
            trace = result.value + ":" + result.done;
        });
        "#,
    )?;

    ensure_value(&context.eval("trace")?, &Value::from("pending"))?;
    context.eval("resolveDelegated(42)")?;
    ensure_value(&context.eval("trace")?, &Value::from("42:false"))
}

#[test]
fn queued_requests_wait_for_the_current_yield_value() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        let yieldOrder = 0;
        let resolveLate;
        function resolveLater() {
            return new Promise(function(resolve) {
                resolveLate = resolve;
            });
        }
        async function* values() {
            yield resolveLater();
            yield ++yieldOrder;
        }
        const iterator = values();
        const first = iterator.next();
        const second = iterator.next();
        const third = iterator.next();
        async function observe() {
            const thirdResult = await third;
            const secondResult = await second;
            const firstResult = await first;
            trace = firstResult.value + ":" + secondResult.value + ":" +
                thirdResult.done + ":" + yieldOrder;
        }
        observe();
        resolveLate(++yieldOrder);
        "#,
    )?;
    let value = context.eval("trace")?;
    ensure_value(&value, &Value::from("1:2:true:2"))
}

#[test]
fn return_resumption_observes_thenable_before_later_promise_jobs() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let events = [];
        async function* values() {
            events.push("start");
            yield 123;
            events.push("unreachable");
        }
        Promise.resolve(0)
            .then(() => events.push("tick 1"))
            .then(() => events.push("tick 2"));
        const iterator = values();
        iterator.next();
        iterator.return({
            get then() {
                events.push("get then");
            }
        });
        "#,
    )?;
    let value = context.eval("events.join('|')")?;
    ensure_value(&value, &Value::from("start|tick 1|get then|tick 2"))
}
