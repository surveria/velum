use velum::{
    Engine, EngineConfig, Error, HostOperation, OptimizationMode, RuntimeLimits, Value, Vm,
    VmConfig, VmRootKind, VmStorageKind, VmStorageLimits,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];

fn configured_vm(mode: OptimizationMode, limits: RuntimeLimits) -> TestResult<Vm> {
    let mut vm = Engine::with_config(EngineConfig::with_default_vm_config(
        VmConfig::with_limits(limits).with_optimization_mode(mode),
    ))
    .create_vm();
    vm.context()
        .register_host_operation("hostGc", HostOperation::CollectGarbage)?;
    vm.context()
        .register_host_operation("hostDda", HostOperation::CreateIsHtmlDda)?;
    Ok(vm)
}

fn ensure_settled(vm: &Vm) -> TestResult {
    let roots = vm.root_snapshot()?;
    for kind in [
        VmRootKind::TransientOperand,
        VmRootKind::TransientCall,
        VmRootKind::TransientTemporary,
    ] {
        if roots.count(kind) != 0 {
            return Err(format!("settled VM retained {kind:?} roots").into());
        }
    }
    if vm.storage_snapshot()?.count(VmStorageKind::TransientRoot) != 0 {
        return Err("settled VM retained transient-root ledger entries".into());
    }
    Ok(())
}

fn check_source(source: &str) -> TestResult {
    for mode in MODES {
        let mut vm = configured_vm(mode, RuntimeLimits::default())?;
        let actual = vm.eval(source)?;
        if actual != Value::Bool(true) {
            return Err(format!("rooting fixture failed in {mode:?}: {actual:?}\n{source}").into());
        }
        ensure_settled(&vm)?;
        vm.collect_garbage()?;
        ensure_settled(&vm)?;
    }
    Ok(())
}

#[test]
fn stack_literals_and_conditional_jumps_keep_prior_operands_at_the_next_call() -> TestResult {
    check_source(
        r#"
        const result = [
            { answer: 42 }, "a\uD800", undefined, !{}, void 1,
            true ? "left" : "right", false || "fallback", null ?? "nullish",
            (hostGc(), 7)
        ];
        result[0].answer === 42 && result[1].charCodeAt(1) === 0xD800 &&
            result[2] === undefined && result[3] === false && result[4] === undefined &&
            result[5] === "left" && result[6] === "fallback" &&
            result[7] === "nullish" && result[8] === 7
        "#,
    )
}

#[test]
fn property_getters_and_proxy_traps_keep_the_outer_operand_stack() -> TestResult {
    check_source(
        r"
        const result = [{ answer: 42 },
            ({ get value() { hostGc(); return 1; } }).value,
            new Proxy({}, { get() { hostGc(); return 2; } }).value
        ];
        result[0].answer === 42 && result[1] === 1 && result[2] === 2
        ",
    )
}

#[test]
fn numeric_specializations_keep_roots_before_object_coercion_fallbacks() -> TestResult {
    check_source(
        r"
        const operand = () => ({ valueOf() { hostGc(); return 2; } });
        const result = [{ answer: 42 },
            +operand(), -operand(), ~operand(), 1 + operand(),
            6 / operand(), 1 < operand(), 2 == operand()
        ];
        result[0].answer === 42 && result[1] === 2 && result[2] === -2 &&
            result[3] === -3 && result[4] === 3 && result[5] === 3 &&
            result[6] && result[7]
        ",
    )
}

#[test]
fn binding_access_and_resolved_assignment_keep_roots_across_reentry() -> TestResult {
    check_source(
        r#"
        Object.defineProperty(globalThis, "indirect", {
            configurable: true,
            get() { hostGc(); return 3; },
            set(value) { hostGc(); globalThis.seen = value; }
        });
        const result = [{ answer: 42 }, indirect, (indirect = { value: 7 })];
        let fromWith;
        with ({ get entry() { hostGc(); return 8; } }) {
            fromWith = [{ answer: 43 }, entry];
        }
        result[0].answer === 42 && result[1] === 3 && result[2] === seen &&
            seen.value === 7 && fromWith[0].answer === 43 && fromWith[1] === 8
        "#,
    )
}

#[test]
fn nested_calls_and_constructors_keep_unbound_arguments() -> TestResult {
    check_source(
        r"
        function consume(first, second) { hostGc(); return first.answer + second; }
        function Box(value) { hostGc(); this.value = value; this.target = new.target; }
        const result = [{ answer: 42 }, consume({ answer: 10 }, 2), new Box({ answer: 30 })];
        result[0].answer === 42 && result[1] === 12 &&
            result[2].value.answer === 30 && result[2].target === Box
        ",
    )
}

#[test]
fn linear_segments_and_structured_loops_preserve_gc_reentry() -> TestResult {
    check_source(
        r"
        const state = { value: 0 };
        let total = 0;
        for (let index = 0; index < 30; index++) {
            const row = [{ answer: index },
                state.value += { valueOf() { hostGc(); return 1; } },
                index < 15 ? 1 : 2];
            total += row[0].answer;
        }
        total === 435 && state.value === 30
        ",
    )
}

#[test]
fn typeof_truthiness_and_delete_value_do_not_invoke_object_hooks() -> TestResult {
    check_source(
        r#"
        let called = 0;
        const hooks = { valueOf() { called++; return 0; }, toString() { called++; return ""; } };
        const revoked = Proxy.revocable(function() {}, {});
        revoked.revoke();
        const dda = hostDda();
        const result = [{ answer: 42 }, typeof revoked.proxy, !hooks, void hooks,
            delete (0, hooks), dda ? 1 : 2, dda || 3, dda ?? 4, (hostGc(), 5)];
        result[0].answer === 42 && result[1] === "function" && result[2] === false &&
            result[3] === undefined && result[4] === true && result[5] === 2 &&
            result[6] === 3 && result[7] === dda && result[8] === 5 && called === 0
        "#,
    )
}

#[test]
fn generator_suspension_preserves_operands_before_and_after_yield() -> TestResult {
    check_source(
        r"
        function* values() {
            const row = [{ answer: 42 }, yield { answer: 9 }, (hostGc(), 1)];
            return row[0].answer + row[1].answer + row[2];
        }
        const iterator = values();
        const first = iterator.next();
        hostGc();
        const second = iterator.next({ answer: 10 });
        first.value.answer === 9 && !first.done && second.done && second.value === 53
        ",
    )
}

#[test]
fn async_suspension_and_jobs_preserve_live_operands() -> TestResult {
    for mode in MODES {
        let mut vm = configured_vm(mode, RuntimeLimits::default())?;
        vm.eval(
            r"
            var result = 0;
            var resolve;
            var pending = new Promise(done => { resolve = done; });
            async function evaluate() {
                const row = [{ answer: 42 }, await pending, (hostGc(), 1)];
                result = row[0].answer + row[1].answer + row[2];
            }
            evaluate();
            ",
        )?;
        vm.collect_garbage()?;
        vm.eval("resolve({ answer: 10 });")?;
        vm.run_jobs()?;
        if vm.eval("result === 53")? != Value::Bool(true) {
            return Err(format!("async operand lifetime failed in {mode:?}").into());
        }
        ensure_settled(&vm)?;
    }
    Ok(())
}

#[test]
fn abrupt_completions_and_disposal_keep_results_and_release_roots() -> TestResult {
    check_source(
        r"
        function finish() {
            using resource = { [Symbol.dispose]() { hostGc(); } };
            return { answer: 42 };
        }
        let caught;
        try { throw { answer: 9 }; }
        catch (error) { hostGc(); caught = error.answer; }
        finally { hostGc(); }
        const result = [{ answer: 1 }, finish(), (hostGc(), 2)];
        result[0].answer === 1 && result[1].answer === 42 && result[2] === 2 && caught === 9
        ",
    )
}

#[test]
fn automatic_collection_at_literal_and_completion_boundaries_keeps_stack_values() -> TestResult {
    for mode in MODES {
        for max_objects in [96, 128, 192] {
            let limits = RuntimeLimits {
                max_objects,
                ..RuntimeLimits::default()
            };
            let mut vm = configured_vm(mode, limits)?;
            let actual = vm.eval(
                r"
                let total = 0;
                for (let index = 0; index < 500; index++) {
                    const row = [{ answer: index }, 1, 2, true ? 3 : 4, undefined];
                    total += row[0].answer;
                }
                total === 124750;
                ",
            )?;
            if actual != Value::Bool(true)
                || vm.storage_snapshot()?.count(VmStorageKind::Object) >= max_objects
            {
                return Err(
                    format!("automatic GC boundary failed: {mode:?}, {max_objects}").into(),
                );
            }
            ensure_settled(&vm)?;
        }
    }
    Ok(())
}

#[test]
fn finite_root_limits_preserve_pure_instruction_rejections() -> TestResult {
    for mode in MODES {
        for (maximum, source) in [(0, "'resident'"), (0, "({})"), (1, "['a', 'b']")] {
            let limits = RuntimeLimits {
                storage: VmStorageLimits::unlimited()
                    .with_max_count(VmStorageKind::TransientRoot, maximum),
                ..RuntimeLimits::default()
            };
            let mut vm = configured_vm(mode, limits)?;
            let Err(error) = vm.eval(source) else {
                return Err(format!("finite root limit {maximum} was bypassed: {source}").into());
            };
            if !matches!(error, Error::ResourceLimit { .. })
                || !error.to_string().contains("TransientRoot")
            {
                return Err(format!("expected transient-root failure, received {error}").into());
            }
            ensure_settled(&vm)?;
            if vm.eval("42")? != Value::Number(42.0) {
                return Err("VM failed to recover after root-budget rejection".into());
            }
        }
    }
    Ok(())
}

#[test]
fn runtime_step_failure_unwinds_roots_after_nonreentrant_instructions() -> TestResult {
    for mode in MODES {
        let limits = RuntimeLimits {
            max_runtime_steps: 128,
            ..RuntimeLimits::default()
        };
        let mut vm = configured_vm(mode, limits)?;
        let Err(error) = vm.eval("let live = {}; while (true) { live; 'text'; 1; 2; }") else {
            return Err("nonreentrant instructions bypassed the step limit".into());
        };
        if !matches!(error, Error::ResourceLimit { .. })
            || !error.to_string().contains("runtime steps")
        {
            return Err(format!("expected runtime step failure, received {error}").into());
        }
        ensure_settled(&vm)?;
    }
    Ok(())
}
