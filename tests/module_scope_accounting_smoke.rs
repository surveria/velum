use velum::{
    HostOperation, ModuleLoader, ModuleSource, OptimizationMode, Value, Vm, VmConfig,
    VmStorageKind, VmStorageSnapshot,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];

#[test]
fn completed_module_scopes_reconcile_bindings_and_caches_before_and_after_gc() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        let baseline = prime_module(&mut vm)?;
        vm.eval_module_named(
            "main.js",
            "export let count = 7; export const label = 'module';",
            &mut FixtureLoader,
        )?;
        let before = vm.storage_snapshot()?;
        ensure_count_growth(
            &before,
            &baseline,
            VmStorageKind::Binding,
            2,
            "completed bindings",
            mode,
        )?;
        ensure_count_growth(
            &before,
            &baseline,
            VmStorageKind::Module,
            1,
            "module owners",
            mode,
        )?;
        ensure_count(
            &before,
            VmStorageKind::ExecutionFrame,
            0,
            "completed frames",
            mode,
        )?;
        if before.count(VmStorageKind::CacheEntry) == 0 {
            return Err(format!("{mode:?}: module cache entries are missing").into());
        }
        vm.collect_garbage()?;
        let after = vm.storage_snapshot()?;
        ensure_count_growth(
            &after,
            &baseline,
            VmStorageKind::Binding,
            2,
            "retained bindings",
            mode,
        )?;
        ensure_count_growth(
            &after,
            &baseline,
            VmStorageKind::Module,
            1,
            "retained modules",
            mode,
        )?;
    }
    Ok(())
}

#[test]
fn canonical_module_aliases_count_each_owning_scope_once() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        let baseline = prime_module(&mut vm)?;
        let source = "import { count, label } from 'values.js'; count + label.length;";
        let first = vm.eval_module_named("first.js", source, &mut FixtureLoader)?;
        ensure_value(&first, &Value::Number(13.0), "first graph", mode)?;
        ensure_count_growth(
            &vm.storage_snapshot()?,
            &baseline,
            VmStorageKind::Binding,
            4,
            "first graph bindings",
            mode,
        )?;
        let second = vm.eval_module_named("second.js", source, &mut FixtureLoader)?;
        ensure_value(&second, &Value::Number(13.0), "aliased graph", mode)?;
        let snapshot = vm.storage_snapshot()?;
        ensure_count_growth(
            &snapshot,
            &baseline,
            VmStorageKind::Binding,
            8,
            "aliased bindings",
            mode,
        )?;
        ensure_count_growth(
            &snapshot,
            &baseline,
            VmStorageKind::Module,
            4,
            "aliased owners",
            mode,
        )?;
        vm.collect_garbage()?;
        ensure_count_growth(
            &vm.storage_snapshot()?,
            &baseline,
            VmStorageKind::Binding,
            8,
            "retained aliases",
            mode,
        )?;
    }
    Ok(())
}

#[test]
fn completed_top_level_await_restores_one_counted_scope() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        let baseline = prime_module(&mut vm)?;
        let value = vm.eval_module_named(
            "main.js",
            "export let value = await Promise.resolve(7); value;",
            &mut FixtureLoader,
        )?;
        ensure_value(&value, &Value::Number(7.0), "awaited value", mode)?;
        let snapshot = vm.storage_snapshot()?;
        ensure_count_growth(
            &snapshot,
            &baseline,
            VmStorageKind::Binding,
            1,
            "awaited bindings",
            mode,
        )?;
        ensure_count(
            &snapshot,
            VmStorageKind::ExecutionFrame,
            0,
            "awaited frames",
            mode,
        )?;
        vm.collect_garbage()?;
        ensure_count_growth(
            &vm.storage_snapshot()?,
            &baseline,
            VmStorageKind::Binding,
            1,
            "retained awaited binding",
            mode,
        )?;
    }
    Ok(())
}

#[test]
fn suspended_top_level_await_does_not_double_count_its_detached_scope() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        let error = match vm.eval_module_named(
            "main.js",
            r"
            export let value = 1;
            await new Promise(function (resolve) { globalThis.resumeModule = resolve; });
            value = 7;
            globalThis.completedModuleValue = value;
            ",
            &mut FixtureLoader,
        ) {
            Ok(value) => {
                return Err(format!("{mode:?}: pending module completed with {value:?}").into());
            }
            Err(error) => error,
        };
        if !error
            .to_string()
            .contains("top-level await remained pending")
        {
            return Err(format!("{mode:?}: unexpected pending module error: {error}").into());
        }
        let suspended = vm.storage_snapshot()?;
        if suspended.count(VmStorageKind::ExecutionFrame) == 0 {
            return Err(format!("{mode:?}: detached module has no execution frame").into());
        }
        vm.collect_garbage()?;
        vm.storage_snapshot()?;
        vm.eval("resumeModule(); 0")?;
        vm.run_jobs()?;
        ensure_value(
            &vm.eval("completedModuleValue")?,
            &Value::Number(7.0),
            "resumed module value",
            mode,
        )?;
        ensure_count(
            &vm.storage_snapshot()?,
            VmStorageKind::ExecutionFrame,
            0,
            "resumed frames",
            mode,
        )?;
        vm.collect_garbage()?;
        vm.storage_snapshot()?;
    }
    Ok(())
}

#[test]
fn failed_module_evaluation_keeps_its_retained_scope_accounted() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        vm.eval("new Error('warmup'); 0")?;
        let baseline = prime_module(&mut vm)?;
        match vm.eval_module_named(
            "main.js",
            "export let value = 7; throw new Error('module failure for test');",
            &mut FixtureLoader,
        ) {
            Ok(value) => return Err(format!("{mode:?}: throwing module returned {value:?}").into()),
            Err(error) if error.to_string().contains("module failure for test") => {}
            Err(error) => {
                return Err(format!("{mode:?}: unexpected module failure: {error}").into());
            }
        }
        let snapshot = vm.storage_snapshot()?;
        ensure_count_growth(
            &snapshot,
            &baseline,
            VmStorageKind::Binding,
            1,
            "failed bindings",
            mode,
        )?;
        ensure_count(
            &snapshot,
            VmStorageKind::ExecutionFrame,
            0,
            "failed frames",
            mode,
        )?;
        vm.collect_garbage()?;
        ensure_count_growth(
            &vm.storage_snapshot()?,
            &baseline,
            VmStorageKind::Binding,
            1,
            "retained failed binding",
            mode,
        )?;
    }
    Ok(())
}

#[test]
fn module_scope_accounting_survives_materializing_and_parking_another_realm() -> TestResult {
    for mode in MODES {
        let mut vm = make_vm(mode);
        vm.context()
            .register_host_operation("createRealmForTest", HostOperation::CreateRealm)?;
        vm.eval_module_named(
            "main.js",
            "export const value = 7; globalThis.moduleValue = value;",
            &mut FixtureLoader,
        )?;
        vm.storage_snapshot()?;
        let value = vm.eval(
            r"
            var other = createRealmForTest();
            other.eval('var value = 19;');
            moduleValue === 7 && other.eval('value') === 19;
            ",
        )?;
        ensure_value(&value, &Value::Bool(true), "parked realm", mode)?;
        vm.storage_snapshot()?;
        vm.collect_garbage()?;
        ensure_count(
            &vm.storage_snapshot()?,
            VmStorageKind::Module,
            1,
            "module retained across realm switch",
            mode,
        )?;
    }
    Ok(())
}

struct FixtureLoader;

impl ModuleLoader for FixtureLoader {
    fn load(&mut self, _referrer: &str, request: &str) -> velum::Result<ModuleSource> {
        if request != "values.js" {
            return Err(velum::Error::runtime(format!(
                "unexpected accounting fixture module '{request}'"
            )));
        }
        Ok(ModuleSource::new(
            request,
            "export let count = 7; export const label = 'module';",
        ))
    }
}

fn make_vm(mode: OptimizationMode) -> Vm {
    Vm::with_config(VmConfig::default().with_optimization_mode(mode))
}

fn prime_module(vm: &mut Vm) -> Result<VmStorageSnapshot, Box<dyn std::error::Error>> {
    // Materialize the common module intrinsics before measuring scope growth.
    vm.eval_module_named("warmup.js", "export const warmup = 0;", &mut FixtureLoader)?;
    Ok(vm.storage_snapshot()?)
}

fn ensure_count_growth(
    snapshot: &VmStorageSnapshot,
    baseline: &VmStorageSnapshot,
    kind: VmStorageKind,
    expected: usize,
    label: &str,
    mode: OptimizationMode,
) -> TestResult {
    let growth = snapshot.count(kind).checked_sub(baseline.count(kind));
    if growth == Some(expected) {
        return Ok(());
    }
    Err(format!(
        "{label} in {mode:?} mode: expected {expected} additional {kind:?}, got {growth:?}"
    )
    .into())
}

fn ensure_count(
    snapshot: &VmStorageSnapshot,
    kind: VmStorageKind,
    expected: usize,
    label: &str,
    mode: OptimizationMode,
) -> TestResult {
    let actual = snapshot.count(kind);
    if actual == expected {
        return Ok(());
    }
    Err(format!("{label} in {mode:?} mode: expected {expected} {kind:?}, got {actual}").into())
}

fn ensure_value(
    actual: &Value,
    expected: &Value,
    label: &str,
    mode: OptimizationMode,
) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("{label} in {mode:?} mode: expected {expected:?}, got {actual:?}").into())
}
