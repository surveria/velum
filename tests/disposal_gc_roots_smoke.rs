use velum::{
    Engine, EngineConfig, Error, HostOperation, OptimizationMode, RuntimeLimits, Value, VmConfig,
    VmRootKind, VmStorageKind, VmStorageLimits,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn check_disposal(source: &str) -> TestResult {
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let mut vm = Engine::with_config(EngineConfig::with_default_vm_config(
            VmConfig::default().with_optimization_mode(mode),
        ))
        .create_vm();
        vm.context()
            .register_host_operation("hostGc", HostOperation::CollectGarbage)?;
        vm.eval(source).map_err(|error| {
            format!("disposal fixture execution failed in {mode:?}: {error:?}\n{source}")
        })?;
        vm.run_jobs()?;
        vm.collect_garbage()?;
        let actual = vm.eval("result")?;
        if actual != Value::Bool(true) {
            return Err(
                format!("disposal fixture failed in {mode:?}: {actual:?}\n{source}").into(),
            );
        }
        let roots = vm.root_snapshot()?;
        for kind in [
            VmRootKind::TransientTemporary,
            VmRootKind::TransientCall,
            VmRootKind::TransientOperand,
        ] {
            if roots.count(kind) != 0 {
                return Err(format!("disposal retained {kind:?} roots in {mode:?}").into());
            }
        }
        let storage = vm.storage_snapshot()?;
        for kind in [VmStorageKind::TransientRoot, VmStorageKind::CollectionEntry] {
            if storage.count(kind) != 0 {
                return Err(format!("disposal retained {kind:?} entries in {mode:?}").into());
            }
        }
    }
    Ok(())
}

#[test]
fn using_preserves_return_and_throw_objects_during_disposer_gc() -> TestResult {
    check_disposal(
        r"
        function finish() {
            using resource = { [Symbol.dispose]() { hostGc(); } };
            return { answer: 42 };
        }
        function fail() {
            using resource = { [Symbol.dispose]() { hostGc(); } };
            throw { answer: 9 };
        }
        let caught;
        try { fail(); } catch (error) { caught = error; }
        const returned = finish();
        hostGc();
        var result = returned.answer === 42 && caught.answer === 9;
        ",
    )
}

#[test]
fn detached_block_resources_keep_their_values_and_methods() -> TestResult {
    check_disposal(
        r"
        let sum = 0;
        {
            using first = { n: 1, [Symbol.dispose]() { hostGc(); sum += this.n; } };
            using second = { n: 2, [Symbol.dispose]() { hostGc(); sum += this.n; } };
            using third = { n: 4, [Symbol.dispose]() { hostGc(); sum += this.n; } };
        }
        var result = sum === 7;
        ",
    )
}

#[test]
fn disposable_stack_keeps_detached_use_adopt_and_defer_resources() -> TestResult {
    check_disposal(
        r"
        let sum = 0;
        const stack = new DisposableStack();
        stack.use({ n: 1, [Symbol.dispose]() { hostGc(); sum += this.n; } });
        stack.adopt({ n: 2 }, value => { hostGc(); sum += value.n; });
        stack.defer((value => () => { hostGc(); sum += value.n; })({ n: 4 }));
        stack.defer(() => { hostGc(); });
        stack.dispose();
        var result = stack.disposed && sum === 7;
        ",
    )
}

#[test]
fn disposable_stack_keeps_new_suppressed_errors_between_callbacks() -> TestResult {
    check_disposal(
        r"
        const stack = new DisposableStack();
        stack.defer(() => { hostGc(); throw { n: 1 }; });
        stack.defer(() => { hostGc(); throw { n: 2 }; });
        stack.defer(() => { hostGc(); throw { n: 3 }; });
        var result = false;
        try { stack.dispose(); } catch (error) {
            hostGc();
            result = error.error.n === 1 && error.suppressed.error.n === 2 &&
                error.suppressed.suppressed.n === 3;
        }
        ",
    )
}

#[test]
fn using_keeps_body_error_and_new_suppression_across_multiple_stacks() -> TestResult {
    check_disposal(
        r"
        var result = false;
        try {
            using first = { [Symbol.dispose]() { hostGc(); throw { n: 1 }; } };
            using second = { [Symbol.dispose]() { hostGc(); throw { n: 2 }; } };
            throw { n: 3 };
        } catch (error) {
            hostGc();
            result = error.error.n === 1 && error.suppressed.error.n === 2 &&
                error.suppressed.suppressed.n === 3;
        }
        ",
    )
}

#[test]
fn await_using_preserves_return_object_before_and_after_suspension() -> TestResult {
    check_disposal(
        r"
        let sum = 0;
        async function finish() {
            await using first = {
                n: 1, [Symbol.asyncDispose]() { hostGc(); sum += this.n; }
            };
            using second = { n: 2, [Symbol.dispose]() { hostGc(); sum += this.n; } };
            await using third = {
                n: 4, [Symbol.asyncDispose]() { hostGc(); sum += this.n; }
            };
            return { answer: 42 };
        }
        var result = false;
        finish().then(value => { hostGc(); result = value.answer === 42 && sum === 7; });
        ",
    )
}

#[test]
fn await_using_preserves_throw_object_and_mixed_stack_errors() -> TestResult {
    check_disposal(
        r"
        async function finish() {
            await using first = {
                [Symbol.asyncDispose]() { hostGc(); throw { n: 1 }; }
            };
            using second = { [Symbol.dispose]() { hostGc(); throw { n: 2 }; } };
            await using third = { [Symbol.asyncDispose]() { hostGc(); } };
            throw { n: 3 };
        }
        var result = false;
        finish().then(undefined, error => {
            hostGc();
            result = error.error.n === 1 && error.suppressed.error.n === 2 &&
                error.suppressed.suppressed.n === 3;
        });
        ",
    )
}

#[test]
fn async_stack_preserves_remaining_resources_during_each_resume() -> TestResult {
    check_disposal(
        r"
        let sum = 0;
        const stack = new AsyncDisposableStack();
        stack.use({ n: 1, [Symbol.asyncDispose]() { hostGc(); sum += this.n; } });
        stack.adopt({ n: 2 }, value => { hostGc(); sum += value.n; });
        stack.defer((value => () => { hostGc(); sum += value.n; })({ n: 4 }));
        stack.defer(() => { hostGc(); });
        var result = false;
        stack.disposeAsync().then(() => { result = stack.disposed && sum === 7; });
        ",
    )
}

#[test]
fn async_stack_preserves_new_errors_before_the_next_await() -> TestResult {
    check_disposal(
        r"
        const stack = new AsyncDisposableStack();
        stack.defer(() => { hostGc(); throw { n: 1 }; });
        stack.defer(() => { hostGc(); throw { n: 2 }; });
        stack.defer(() => { hostGc(); throw { n: 3 }; });
        var result = false;
        stack.disposeAsync().then(undefined, error => {
            hostGc();
            result = error.error.n === 1 && error.suppressed.error.n === 2 &&
                error.suppressed.suppressed.n === 3;
        });
        ",
    )
}

#[test]
fn async_stack_roots_the_unobserved_result_promise_while_resuming() -> TestResult {
    check_disposal(
        r"
        var result = false;
        (function() {
            const stack = new AsyncDisposableStack();
            stack.defer(() => { hostGc(); result = true; });
            stack.defer(() => undefined);
            stack.disposeAsync();
        })();
        ",
    )
}

#[test]
fn promise_handlers_keep_their_result_resolvers_during_gc() -> TestResult {
    check_disposal(
        r"
        var result = false;
        Promise.resolve(1).then(value => {
            hostGc();
            return { n: value + 1 };
        }).then(value => {
            hostGc();
            throw { n: value.n + 1 };
        }).then(undefined, error => {
            hostGc();
            result = error.n === 3;
        });
        ",
    )
}

#[test]
fn tail_call_operands_survive_using_disposal() -> TestResult {
    check_disposal(
        r#"
        function finish() {
            "use strict";
            using resource = { [Symbol.dispose]() { hostGc(); } };
            return (function(value) { hostGc(); return value; })({ n: 42 });
        }
        const value = finish();
        hostGc();
        var result = value.n === 42;
        "#,
    )
}

#[test]
fn disposal_root_limit_rejects_before_detaching_stack_resources() -> TestResult {
    let limits = RuntimeLimits {
        storage: VmStorageLimits::unlimited().with_max_count(VmStorageKind::TransientRoot, 64),
        ..RuntimeLimits::default()
    };
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let mut vm = Engine::with_config(EngineConfig::with_default_vm_config(
            VmConfig::with_limits(limits.clone()).with_optimization_mode(mode),
        ))
        .create_vm();
        vm.eval(
            "const stack = new DisposableStack();
             for (let i = 0; i < 100; i++) { stack.adopt({ n: i }, value => value.n); }",
        )?;
        let Err(error) = vm.eval("stack.dispose()") else {
            return Err("disposal ignored the transient-root limit".into());
        };
        if !matches!(error, Error::ResourceLimit { .. })
            || !error.to_string().contains("TransientRoot")
        {
            return Err(format!("unexpected disposal root error: {error}").into());
        }
        vm.collect_garbage()?;
        let storage = vm.storage_snapshot()?;
        if vm.eval("stack.disposed")? != Value::Bool(false)
            || storage.count(VmStorageKind::CollectionEntry) != 100
            || storage.count(VmStorageKind::TransientRoot) != 0
            || vm.eval("42")? != Value::Number(42.0)
        {
            return Err(format!("root-budget rejection corrupted the stack in {mode:?}").into());
        }
    }
    Ok(())
}
