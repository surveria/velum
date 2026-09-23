use velum::{
    HostOperation, ModuleLoader, ModuleSource, OptimizationMode, Runtime, RuntimeLimits, Value, Vm,
    VmConfig, VmStorageKind,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];

fn make_vm(mode: OptimizationMode) -> TestResult<Vm> {
    let mut vm = Vm::with_config(VmConfig::default().with_optimization_mode(mode));
    vm.context()
        .register_host_operation("hostGc", HostOperation::CollectGarbage)?;
    Ok(vm)
}

fn ensure_true(actual: &Value, mode: OptimizationMode, source: &str) -> TestResult {
    if actual != &Value::Bool(true) {
        return Err(
            format!("binding read in {mode:?}: expected true, got {actual:?}\n{source}").into(),
        );
    }
    Ok(())
}

fn ensure_settled(vm: &mut Vm, mode: OptimizationMode) -> TestResult {
    vm.collect_garbage()?;
    let snapshot = vm.storage_snapshot()?;
    for kind in [VmStorageKind::TransientRoot, VmStorageKind::ExecutionFrame] {
        if snapshot.count(kind) != 0 {
            return Err(
                format!("binding read in {mode:?}: retained {kind:?} after execution").into(),
            );
        }
    }
    Ok(())
}

fn check_script(source: &str) -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode)?;
        ensure_true(&vm.eval(source)?, mode, source)?;
        ensure_settled(&mut vm, mode)?;
    }
    Ok(())
}

#[test]
fn initialized_reads_preserve_value_variants_and_identity() -> TestResult {
    check_script(
        r#"
        const values = [undefined, null, false, true, -0, NaN, Infinity, 42,
            12345678901234567890n, "a\uD800b", Symbol("key"), { answer: 42 },
            function f() {}, Math.abs, class C {}, new Proxy({}, {})];
        function read(input) {
            let local = input;
            function captured() { hostGc(); return local; }
            return Object.is(input, local) && Object.is(input, captured());
        }
        values.every(read)
        "#,
    )
}

#[test]
fn binding_reads_release_borrows_before_coercion_and_reentry() -> TestResult {
    check_script(
        r"
        function run() {
            let value = {
                valueOf() { value = 40; hostGc(); return 2; }
            };
            const sum = value + 1;
            let current = {
                get answer() { current = { answer: 43 }; hostGc(); return 42; }
            };
            const first = current.answer;
            return sum === 3 && value === 40 && first === 42 && current.answer === 43;
        }
        run() && run()
        ",
    )
}

#[test]
fn captured_cells_observe_updates_across_calls_and_garbage_collection() -> TestResult {
    check_script(
        r"
        function make(seed) {
            let value = { answer: seed };
            return {
                read() { hostGc(); return value; },
                write(next) { value = next; }
            };
        }
        const left = make(1), right = make(10);
        const first = left.read();
        left.write({ answer: 42 });
        hostGc();
        first.answer === 1 && left.read().answer === 42 && right.read().answer === 10
        ",
    )
}

#[test]
fn tdz_reads_and_typeof_keep_reference_errors_before_initialization() -> TestResult {
    check_script(
        r"
        let errors = 0;
        {
            try { value; } catch (e) { if (e instanceof ReferenceError) errors++; }
            try { typeof value; } catch (e) { if (e instanceof ReferenceError) errors++; }
            let value = 42;
            if (value !== 42) errors = -100;
        }
        function parameter(first = second, second = 1) { return first; }
        try { parameter(); } catch (e) { if (e instanceof ReferenceError) errors++; }
        errors === 3 && typeof missingBindingForReadTest === 'undefined'
        ",
    )
}

#[test]
fn deleted_eval_bindings_and_redeclarations_remain_live() -> TestResult {
    check_script(
        r#"
        var value = 100;
        function make() {
            var read = function() { return value; };
            eval("var value = 1;");
            if (read() !== 1) return false;
            if (!eval("delete value")) return false;
            if (read() !== 100) return false;
            eval("var value = 42;");
            hostGc();
            return read() === 42;
        }
        make() && make() && value === 100
        "#,
    )
}

#[test]
fn dynamic_binding_reads_keep_accessors_and_unscopables() -> TestResult {
    check_script(
        r"
        var item = 40;
        var reads = 0;
        const scope = {
            get item() { reads++; hostGc(); return 2; },
            [Symbol.unscopables]: { item: false }
        };
        let read;
        with (scope) { read = function() { return item; }; }
        const first = read();
        scope[Symbol.unscopables].item = true;
        const second = read();
        Object.defineProperty(globalThis, 'indirectRead', {
            configurable: true, get() { hostGc(); return ++reads; }
        });
        first === 2 && second === 40 && indirectRead === 2 && indirectRead === 3
        ",
    )
}

#[test]
fn immutable_function_and_global_bindings_keep_their_assignment_rules() -> TestResult {
    check_script(
        r"
        var named = function inner() {
            var original = inner;
            inner = 1;
            return inner === original;
        };
        var strictNamed = function strictInner() {
            'use strict';
            try { strictInner = 1; } catch (e) { return e instanceof TypeError; }
            return false;
        };
        undefined = 42;
        named() && strictNamed() && undefined === void 0
        ",
    )
}

#[test]
fn compiled_binding_reads_remain_isolated_across_vms_and_runs() -> TestResult {
    const SOURCE: &str = r"
        function count(seed) {
            let total = seed;
            for (let index = 0; index < 10; index++) total += index;
            return total;
        }
        count(input) === input + 45
    ";
    let runtime = Runtime::new();
    let script = runtime.compile(SOURCE)?;
    for mode in MODES {
        for initial in [1, 100] {
            let mut vm = make_vm(mode)?;
            vm.eval(&format!("var input = {initial};"))?;
            for _ in 0..3 {
                ensure_true(&vm.eval_compiled(&script)?, mode, SOURCE)?;
                vm.eval("input += 1;")?;
            }
            ensure_settled(&mut vm, mode)?;
        }
    }
    Ok(())
}

#[test]
fn live_import_reads_observe_mutations_through_reexports_and_namespace() -> TestResult {
    check_module(
        r"
        import { item, replace } from 'bridge.js';
        import * as namespace from 'values.js';
        const initial = item;
        replace({ answer: 42 });
        hostGc();
        initial.answer === 1 && item.answer === 42 && namespace.item === item
        ",
    )
}

#[test]
fn cyclic_import_reads_preserve_tdz_and_later_initialization() -> TestResult {
    check_module(
        r"
        import { early, late, value } from 'cycle-a.js';
        hostGc();
        early === 'ReferenceError' && late === 42 && value === 42
        ",
    )
}

#[test]
fn imports_remain_immutable_when_their_targets_change() -> TestResult {
    check_module(
        r"
        import { item, replace } from 'values.js';
        let rejected = false;
        try { item = 7; } catch (e) { rejected = e instanceof TypeError; }
        replace({ answer: 43 });
        hostGc();
        rejected && item.answer === 43
        ",
    )
}

#[test]
fn binding_read_loops_still_exhaust_execution_limits() -> TestResult {
    for mode in MODES {
        let limits = RuntimeLimits {
            max_runtime_steps: 128,
            ..RuntimeLimits::default()
        };
        let mut vm = Vm::with_config(VmConfig::with_limits(limits).with_optimization_mode(mode));
        let error = vm
            .eval(
                "let total = 0; for (let index = 0; index < 10000; index++) total += index; total;",
            )
            .err()
            .ok_or("binding loop unexpectedly bypassed the execution limit")?;
        if !matches!(error, velum::Error::ResourceLimit { .. }) {
            return Err(format!("{mode:?}: expected execution limit, got {error}").into());
        }
    }
    Ok(())
}

fn check_module(source: &str) -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode)?;
        ensure_true(
            &vm.eval_module_named("main.js", source, &mut FixtureLoader)?,
            mode,
            source,
        )?;
        ensure_settled(&mut vm, mode)?;
    }
    Ok(())
}

struct FixtureLoader;

impl ModuleLoader for FixtureLoader {
    fn load(&mut self, _referrer: &str, request: &str) -> velum::Result<ModuleSource> {
        let source = match request {
            "values.js" => {
                "export let item = { answer: 1 }; export function replace(next) { item = next; }"
            }
            "bridge.js" => "export { item, replace } from 'values.js';",
            "cycle-a.js" => {
                "import { read } from 'cycle-b.js'; export const early = read(); \
                 export let value = 42; export const late = read();"
            }
            "cycle-b.js" => {
                "import { value } from 'cycle-a.js'; \
                 export function read() { try { return value; } catch (error) { return error.name; } }"
            }
            _ => {
                return Err(velum::Error::runtime(format!(
                    "unexpected read fixture '{request}'"
                )));
            }
        };
        Ok(ModuleSource::new(request, source))
    }
}
