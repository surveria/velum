use rs_quickjs::{Runtime, Value, Vm, VmAsyncEdgeKind, VmStorageKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_promise_constructor_and_methods() -> TestResult {
    let value = eval(
        r"
        typeof Promise === 'function' &&
        Promise.name === 'Promise' &&
        Promise.length === 1 &&
        typeof Promise.all === 'function' &&
        Promise.all.length === 1 &&
        typeof Promise.resolve === 'function' &&
        Promise.resolve.length === 1 &&
        typeof Promise.reject === 'function' &&
        Promise.reject.length === 1 &&
        typeof Promise.prototype.then === 'function' &&
        Promise.prototype.then.length === 2 &&
        typeof Promise.prototype.catch === 'function' &&
        Promise.prototype.catch.length === 1 &&
        Promise.prototype.constructor === Promise
        ",
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn promise_all_preserves_order_and_rejects_on_input_failure() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let observed = "pending";
        let rejected = "pending";
        let empty = "pending";
        Promise.all([
            Promise.resolve(1),
            2,
            { then(resolve) { resolve(3); } }
        ]).then(function(values) {
            observed = values.join(",");
        });
        Promise.all([1, Promise.reject("bad")]).then(
            function() { rejected = "fulfilled"; },
            function(reason) { rejected = reason; }
        );
        Promise.all([]).then(function(values) {
            empty = values.length === 0 ? "empty" : "not-empty";
        });
        "#,
    )?;

    let value = context.eval("observed + '|' + rejected + '|' + empty")?;
    ensure_value(&value, &Value::from("1,2,3|bad|empty"))
}

#[test]
fn drains_resolved_promise_then_jobs_after_eval() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let value = 0;
        Promise.resolve(40).then(function(resolved) {
            value = resolved + 2;
        });
        ",
    )?;

    let value = context.eval("value")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn propagates_rejected_promise_to_catch_handler() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let reason = "";
        Promise.reject("offline").catch(function(error) {
            reason = error;
        });
        "#,
    )?;

    let value = context.eval("reason")?;
    ensure_value(&value, &Value::from("offline"))
}

#[test]
fn async_function_returns_a_resolved_promise() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        async function answer() {
            return 42;
        }
        let value = 0;
        answer().then(function(resolved) {
            value = resolved;
        });
        ",
    )?;

    let value = context.eval("value")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn exposes_async_function_constructor_and_prototype() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let AsyncFunction = async function() {}.constructor;
        let AsyncFunctionPrototype = AsyncFunction.prototype;
        let first = AsyncFunction("await 1");
        let second = new AsyncFunction("left", "right", "return await left + right;");
        typeof AsyncFunction === "function" &&
            AsyncFunction.name === "AsyncFunction" &&
            AsyncFunction.length === 1 &&
            Object.getPrototypeOf(async function() {}) === AsyncFunctionPrototype &&
            AsyncFunctionPrototype.constructor === AsyncFunction &&
            first.constructor === AsyncFunction &&
            first.length === 0 &&
            Object.getPrototypeOf(first) === AsyncFunctionPrototype &&
            second.constructor === AsyncFunction &&
            second.length === 2 &&
            Object.getPrototypeOf(second) === AsyncFunctionPrototype ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn await_reads_already_resolved_promise_value() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        async function answer() {
            let base = await Promise.resolve(40);
            return base + 2;
        }
        let value = 0;
        answer().then(function(resolved) {
            value = resolved;
        });
        ",
    )?;

    let value = context.eval("value")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn pending_await_resumes_after_later_resolution() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let resolveLater;
        let afterAwait = false;
        let result = 0;

        async function task() {
            let value = await new Promise(function(resolve) {
                resolveLater = resolve;
            });
            afterAwait = true;
            return value + 1;
        }

        task().then(function(value) {
            result = value;
        });
        ",
    )?;

    ensure_value(&context.eval("afterAwait")?, &Value::Bool(false))?;
    context.eval("resolveLater(41)")?;
    ensure_value(&context.eval("afterAwait")?, &Value::Bool(true))?;
    ensure_value(&context.eval("result")?, &Value::Number(42.0))
}

#[test]
fn async_function_can_suspend_more_than_once() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let resolveFirst;
        let resolveSecond;
        let stage = 0;
        let result = 0;
        let first = new Promise(function(resolve) { resolveFirst = resolve; });
        let second = new Promise(function(resolve) { resolveSecond = resolve; });

        async function task() {
            let left = await first;
            stage = 1;
            let right = await second;
            stage = 2;
            return left + right;
        }

        task().then(function(value) {
            result = value;
        }, function(error) {
            result = error.name + ':' + error.message;
        });
        ",
    )?;

    context.eval("resolveFirst(20)")?;
    ensure_value(&context.eval("stage")?, &Value::Number(1.0))?;
    ensure_value(&context.eval("result")?, &Value::Number(0.0))?;
    context.eval("resolveSecond(22)")?;
    ensure_value(&context.eval("stage")?, &Value::Number(2.0))?;
    ensure_value(&context.eval("result")?, &Value::Number(42.0))
}

#[test]
fn rejection_after_repeated_suspension_uses_the_shared_completion_path() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let rejectSecond;
        let stage = 0;
        let result = "pending";

        async function task() {
            await Promise.resolve(1);
            stage = 1;
            await new Promise(function(resolve, reject) {
                rejectSecond = reject;
            });
            stage = 2;
        }

        task().then(
            function() { result = "fulfilled"; },
            function(reason) { result = reason; }
        );
        "#,
    )?;

    ensure_value(&context.eval("stage")?, &Value::Number(1.0))?;
    ensure_value(&context.eval("result")?, &Value::from("pending"))?;
    context.eval("rejectSecond('second await')")?;
    ensure_value(&context.eval("stage")?, &Value::Number(1.0))?;
    ensure_value(&context.eval("result")?, &Value::from("second await"))
}

#[test]
fn rejected_await_rejects_the_async_function_promise() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let rejectLater;
        let reason = "";
        async function task() {
            await new Promise(function(resolve, reject) {
                rejectLater = reject;
            });
            reason = "continued";
        }
        task().catch(function(error) { reason = error; });
        "#,
    )?;

    context.eval("rejectLater('offline')")?;
    ensure_value(&context.eval("reason")?, &Value::from("offline"))
}

#[test]
fn pending_await_resumes_inside_structured_control() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let resolveLater;
        let stage = 0;
        async function task() {
            let index = 0;
            while (index < 2) {
                index = index + 1;
                let value = await new Promise(function(resolve) {
                    resolveLater = resolve;
                });
                stage = stage + value;
            }
            return stage;
        }
        let result = 0;
        task().then(function(value) {
            result = value;
        }, function(error) {
            result = error.name + ':' + error.message;
        });
        ",
    )?;

    context.eval("resolveLater(20)")?;
    ensure_value(&context.eval("stage")?, &Value::Number(20.0))?;
    context.eval("resolveLater(22)")?;
    ensure_value(&context.eval("stage")?, &Value::Number(42.0))?;
    ensure_value(&context.eval("result")?, &Value::Number(42.0))
}

#[test]
fn await_resumes_across_structured_control_kinds() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let result = 0;
        async function task() {
            let total = 0;
            for (let index = 0; index < 2; index = index + 1) {
                total = total + await Promise.resolve(5);
            }
            for (let value of [1, 2]) {
                total = total + await Promise.resolve(value);
            }
            for (let key in { left: 1 }) {
                total = total + await Promise.resolve(key === 'left' ? 3 : 0);
            }
            switch (1) {
                case await Promise.resolve(1):
                    let switchScoped = 4;
                    total = total + await Promise.resolve(switchScoped);
                    break;
                default:
                    total = -100;
            }
            try {
                total = total + await Promise.resolve(10);
            } finally {
                total = total + await Promise.resolve(12);
            }
            return total;
        }
        task().then(function(value) { result = value; });
        ",
    )?;

    ensure_value(&context.eval("result")?, &Value::Number(42.0))
}

#[test]
fn await_resumes_inside_nested_expressions_and_patterns() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let result = 0;
        let keyReads = 0;
        function key() {
            keyReads = keyReads + 1;
            return 'head';
        }
        async function task() {
            let { [key()]: head, tail = await Promise.resolve(2) } = { head: 40 };
            let [left = await Promise.resolve(20)] = [undefined];
            let right = false || await Promise.resolve(22);
            let fallback = null ?? await Promise.resolve(0);
            let forOfTotal = 0;
            for (let [item = await Promise.resolve(21)] of [[], []]) {
                forOfTotal = forOfTotal + item;
            }
            let forInTotal = 0;
            for (let { missing = await Promise.resolve(21) } in { a: 1, b: 2 }) {
                forInTotal = forInTotal + missing;
            }
            return left + right + fallback + head + tail + forOfTotal +
                forInTotal - 126 +
                (keyReads === 1 ? 0 : 1000);
        }
        task().then(function(value) { result = value; });
        ",
    )?;

    ensure_value(&context.eval("result")?, &Value::Number(42.0))
}

#[test]
fn rejected_await_can_resume_catch_and_async_finally() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let result = "";
        async function task() {
            try {
                await Promise.reject("offline");
                return "unreachable";
            } catch (error) {
                try {
                    return error + await Promise.resolve("-caught");
                } finally {
                    await Promise.resolve("cleanup");
                }
            }
        }
        task().then(
            function(value) { result = value; },
            function(error) { result = error.name + ":" + error.message; }
        );
        "#,
    )?;

    ensure_value(&context.eval("result")?, &Value::from("offline-caught"))
}

#[test]
fn await_resumes_catch_parameter_destructuring_in_the_existing_scope() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let result = "pending";
        async function task() {
            try {
                throw { left: 20 };
            } catch ({ left, right = await Promise.resolve(22) }) {
                return left + right;
            }
        }
        task().then(
            function(value) { result = value; },
            function(error) { result = error.name + ":" + error.message; }
        );
        "#,
    )?;

    ensure_value(&context.eval("result")?, &Value::Number(42.0))
}

#[test]
fn embedder_can_cancel_parked_async_jobs() -> TestResult {
    let mut vm = Vm::new();
    vm.eval(
        r"
        let resolveLater;
        let resumed = false;
        async function task() {
            await new Promise(function(resolve) { resolveLater = resolve; });
            resumed = true;
        }
        task().then(function() { resumed = 'settled'; });
        ",
    )?;

    let before = vm.storage_snapshot()?;
    if before.count(VmStorageKind::ExecutionFrame) == 0
        || before.count(VmStorageKind::PromiseReaction) == 0
    {
        return Err("expected parked async storage before cancellation".into());
    }
    if vm
        .async_edge_snapshot()?
        .count(VmAsyncEdgeKind::PromiseReaction)
        == 0
    {
        return Err("expected parked async Promise-reaction edges".into());
    }
    let cancelled = vm.cancel_jobs()?;
    if cancelled == 0 {
        return Err("expected at least one cancelled Promise reaction".into());
    }
    if vm.pending_job_count() != 0 {
        return Err("expected an empty ready-job queue after cancellation".into());
    }
    let after = vm.storage_snapshot()?;
    if after.count(VmStorageKind::ExecutionFrame) != 0 {
        return Err("expected cancellation to release parked execution frames".into());
    }
    if after.count(VmStorageKind::PromiseReaction) != 0 {
        return Err("expected cancellation to release Promise reactions".into());
    }
    if vm
        .async_edge_snapshot()?
        .count(VmAsyncEdgeKind::PromiseReaction)
        != 0
    {
        return Err("expected cancellation to release async reaction edges".into());
    }
    vm.eval("resolveLater(1)")?;
    ensure_value(&vm.eval("resumed")?, &Value::Bool(false))
}

#[test]
fn unsupported_top_level_await_does_not_leak_execution_frames() -> TestResult {
    let mut vm = Vm::new();
    let error = vm
        .eval("await Promise.resolve(1)")
        .err()
        .ok_or("expected top-level await to require an async evaluation API")?;
    if !error
        .to_string()
        .contains("top-level await requires an asynchronous evaluation API")
    {
        return Err(format!("unexpected top-level await error: {error}").into());
    }
    let snapshot = vm.storage_snapshot()?;
    if snapshot.count(VmStorageKind::ExecutionFrame) != 0 {
        return Err("top-level await retained execution frames after rejection".into());
    }
    Ok(())
}

#[test]
fn await_resumes_in_a_later_promise_job() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let completion = context.eval(
        r#"
        let events = "";
        async function task() {
            events = events + "a";
            await Promise.resolve(1);
            events = events + "c";
        }
        task();
        events = events + "b";
        events;
        "#,
    )?;

    ensure_value(&completion, &Value::from("ab"))?;
    ensure_value(&context.eval("events")?, &Value::from("abc"))
}

#[test]
fn async_function_rejects_promise_constructor_type_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let started = false;
        let afterAwait = false;
        let rejected = "";

        async function task() {
            started = true;
            await new Promise();
            afterAwait = true;
        }

        task().then(function() {
            rejected = "resolved";
        }, function(error) {
            rejected = error.name + ":" + error.message;
        });
        "#,
    )?;

    let value = context.eval(
        r#"
        started &&
            !afterAwait &&
            rejected === "TypeError:Promise constructor requires an executor" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

fn eval(source: &str) -> rs_quickjs::Result<Value> {
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
