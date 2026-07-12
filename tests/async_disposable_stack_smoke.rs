use rs_quickjs::{Runtime, Value, VmAsyncEdgeKind, VmStorageKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn ensure_string_value(actual: &Value, expected: &str) -> TestResult {
    let actual = match actual {
        Value::String(value) => value.as_str(),
        Value::HeapString(value) => value.as_str(),
        other => return Err(format!("expected string {expected:?}, got {other:?}").into()),
    };
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected string {expected:?}, got {actual:?}").into())
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)?;
    ensure_string_value(&context.eval("result")?, expected)
}

#[test]
fn async_disposable_stack_exposes_metadata_and_lifo_semantics() -> TestResult {
    ensure_string(
        r#"
        const seen = [];
        const asyncResource = {
            [Symbol.asyncDispose]() {
                seen.push("async-use");
                return Promise.resolve();
            }
        };
        const syncResource = {
            [Symbol.dispose]() { seen.push("sync-use"); }
        };
        const stack = new AsyncDisposableStack();
        stack.use(asyncResource);
        stack.use(syncResource);
        stack.adopt(7, value => seen.push("adopt:" + value));
        stack.defer(() => seen.push("defer"));
        const moved = stack.move();
        let result = "pending";
        moved.disposeAsync().then(() => {
            result = [
                AsyncDisposableStack.name,
                AsyncDisposableStack.length,
                stack.disposed,
                moved.disposed,
                moved[Symbol.asyncDispose] === AsyncDisposableStack.prototype.disposeAsync,
                Object.prototype.toString.call(moved),
                seen.join(",")
            ].join("|");
        });
        result
        "#,
        "AsyncDisposableStack|0|true|true|true|[object AsyncDisposableStack]|defer,adopt:7,sync-use,async-use",
    )
}

#[test]
fn async_disposal_awaits_in_order_and_nests_throw_values() -> TestResult {
    ensure_string(
        r#"
        const first = {};
        const second = {};
        const third = {};
        const stack = new AsyncDisposableStack();
        stack.defer(() => { throw first; });
        stack.defer(() => Promise.reject(second));
        stack.defer(() => { throw third; });
        let result = "pending";
        stack.disposeAsync().then(
            () => { result = "did-not-reject"; },
            error => {
                result = [
                    error instanceof SuppressedError,
                    error.error === first,
                    error.suppressed instanceof SuppressedError,
                    error.suppressed.error === second,
                    error.suppressed.suppressed === third
                ].join(":");
            }
        );
        result
        "#,
        "true:true:true:true:true",
    )
}

#[test]
fn dispose_async_rejects_invalid_receivers_without_throwing_synchronously() -> TestResult {
    ensure_string(
        r#"
        const disposeAsync = AsyncDisposableStack.prototype.disposeAsync;
        let synchronous = false;
        let promise;
        try { promise = disposeAsync.call({}); }
        catch (_) { synchronous = true; }
        let result = "pending";
        promise.then(
            () => { result = "fulfilled"; },
            error => {
                result = [synchronous, error instanceof TypeError ? "type" : "other"].join(":");
            }
        );
        [synchronous, result].join(":")
        "#,
        "false:type",
    )
}

#[test]
fn parked_async_disposal_survives_gc_and_releases_resource_storage() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        globalThis.disposalLog = [];
        globalThis.resolveDisposal = undefined;
        const stack = new AsyncDisposableStack();
        stack.defer(() => disposalLog.push("bottom"));
        stack.defer(() => new Promise(resolve => {
            disposalLog.push("parked");
            globalThis.resolveDisposal = resolve;
        }));
        stack.disposeAsync().then(() => disposalLog.push("done"));
        "#,
    )?;
    ensure_string_value(&context.eval("disposalLog.join(',')")?, "parked")?;
    if context
        .async_edge_snapshot()?
        .count(VmAsyncEdgeKind::PromiseReaction)
        == 0
    {
        return Err("expected parked disposal Promise-reaction edges".into());
    }
    context.collect_garbage()?;
    context.eval("globalThis.resolveDisposal()")?;
    ensure_string_value(
        &context.eval("disposalLog.join(',')")?,
        "parked,bottom,done",
    )?;
    let entries = context
        .storage_snapshot()?
        .count(VmStorageKind::CollectionEntry);
    if entries == 0 {
        return Ok(());
    }
    Err(format!("expected disposal to release collection entries, got {entries}").into())
}
