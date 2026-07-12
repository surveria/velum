use rs_quickjs::{Runtime, Value, VmStorageKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    ensure_value(&context.eval(source)?, &Value::String(expected.to_owned()))
}

#[test]
fn disposable_stack_exposes_spec_metadata_and_lifo_semantics() -> TestResult {
    ensure_string(
        r#"
        const seen = [];
        const resource = { [Symbol.dispose]() { seen.push("use"); } };
        const stack = new DisposableStack();
        stack.use(resource);
        stack.adopt(7, value => seen.push("adopt:" + value));
        stack.defer(() => seen.push("defer"));
        const moved = stack.move();
        moved.dispose();
        [
            DisposableStack.name,
            DisposableStack.length,
            stack.disposed,
            moved.disposed,
            moved[Symbol.dispose] === DisposableStack.prototype.dispose,
            Object.prototype.toString.call(moved),
            seen.join(",")
        ].join("|")
        "#,
        "DisposableStack|0|true|true|true|[object DisposableStack]|defer,adopt:7,use",
    )
}

#[test]
fn disposal_preserves_and_nests_javascript_throw_values() -> TestResult {
    ensure_string(
        r#"
        const first = {};
        const second = {};
        const third = {};
        const stack = new DisposableStack();
        stack.defer(() => { throw first; });
        stack.defer(() => { throw second; });
        stack.defer(() => { throw third; });
        try {
            stack.dispose();
            "did-not-throw";
        } catch (error) {
            [
                error instanceof SuppressedError,
                error.error === first,
                error.suppressed instanceof SuppressedError,
                error.suppressed.error === second,
                error.suppressed.suppressed === third
            ].join(":");
        }
        "#,
        "true:true:true:true:true",
    )
}

#[test]
fn disposable_resources_survive_gc_and_release_storage_on_dispose() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        globalThis.disposalLog = [];
        globalThis.stack = new DisposableStack();
        stack.defer(() => disposalLog.push("first"));
        stack.defer(() => disposalLog.push("second"));
        "#,
    )?;
    let before_dispose = context
        .storage_snapshot()?
        .count(VmStorageKind::CollectionEntry);
    if before_dispose < 2 {
        return Err(
            format!("expected at least two retained resources, got {before_dispose}").into(),
        );
    }
    context.collect_garbage()?;
    ensure_value(
        &context.eval("stack.dispose(); disposalLog.join(',')")?,
        &Value::String("second,first".to_owned()),
    )?;
    let after_dispose = context
        .storage_snapshot()?
        .count(VmStorageKind::CollectionEntry);
    if after_dispose.checked_add(2) == Some(before_dispose) {
        return Ok(());
    }
    Err(format!(
        "expected disposal to release two collection entries, before={before_dispose}, after={after_dispose}"
    )
    .into())
}

#[test]
fn suppressed_error_defines_non_enumerable_payload_fields() -> TestResult {
    ensure_string(
        r#"
        const error = {};
        const suppressed = {};
        const value = new SuppressedError(error, suppressed, "message");
        const errorDescriptor = Object.getOwnPropertyDescriptor(value, "error");
        const suppressedDescriptor = Object.getOwnPropertyDescriptor(value, "suppressed");
        [
            value.name,
            value.message,
            value.error === error,
            value.suppressed === suppressed,
            errorDescriptor.writable,
            !errorDescriptor.enumerable,
            errorDescriptor.configurable,
            !suppressedDescriptor.enumerable
        ].join(":")
        "#,
        "SuppressedError:message:true:true:true:true:true:true",
    )
}

#[test]
fn disposable_stack_reads_bound_new_target_prototype_before_allocation() -> TestResult {
    ensure_string(
        r#"
        const custom = function() {}.bind(null);
        let reads = 0;
        Object.defineProperty(custom, "prototype", {
            get() { reads += 1; return Array.prototype; }
        });
        const stack = Reflect.construct(DisposableStack, [], custom);
        [Object.getPrototypeOf(stack) === Array.prototype, reads].join(":")
        "#,
        "true:1",
    )
}

#[test]
fn bound_prototype_accessors_preserve_existing_constructor_ordering() -> TestResult {
    ensure_string(
        r#"
        const promiseTarget = function() {}.bind(null);
        let promisePrototypeRead = false;
        Object.defineProperty(promiseTarget, "prototype", {
            get() { promisePrototypeRead = true; throw new Error("unexpected"); }
        });
        let promiseError = "none";
        try { Reflect.construct(Promise, [], promiseTarget); }
        catch (error) { promiseError = error instanceof TypeError ? "type" : "other"; }

        const pattern = /a/;
        const regexpTarget = function() {}.bind(null);
        Object.defineProperty(regexpTarget, "prototype", {
            get() {
                pattern.compile("b");
                return RegExp.prototype;
            }
        });
        const copy = Reflect.construct(RegExp, [pattern], regexpTarget);
        [promiseError, promisePrototypeRead, copy.source, pattern.source].join(":")
        "#,
        "type:false:a:b",
    )
}
