use velum::{
    Engine, EngineConfig, Error, OptimizationMode, RuntimeLimits, Value, Vm, VmConfig,
    VmStorageKind, VmStorageLimits,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];
const PROPERTY_KIND: VmStorageKind = VmStorageKind::ObjectProperty;

#[test]
fn packed_unshift_reserves_all_properties_before_mutating() -> TestResult {
    check_transactional_unshift(
        "packed unshift",
        "var values = [3, 4]; values.unshift; values.shift;",
        "values.length === 2 && values[0] === 3 && values[1] === 4 && !(2 in values)",
        "values.length === 3 && values[0] === 1 && values[1] === 3 && values[2] === 4",
    )
}

#[test]
fn holey_unshift_reserves_before_moving_present_elements_or_holes() -> TestResult {
    check_transactional_unshift(
        "holey unshift",
        "var values = [, 3, , 4]; values.unshift; values.shift;",
        "values.length === 4 && !(0 in values) && values[1] === 3 && \
         !(2 in values) && values[3] === 4 && !(4 in values)",
        "values.length === 5 && values[0] === 1 && !(1 in values) && \
         values[2] === 3 && !(3 in values) && values[4] === 4",
    )
}

#[test]
fn unshift_fallback_preserves_non_default_and_sparse_properties() -> TestResult {
    const SOURCE: &str = r"
        var values = [3, 4];
        Object.defineProperty(values, '0', {enumerable: false});
        var length = values.unshift(1, 2);
        var sparse = [];
        Object.defineProperty(sparse, '4096', {
            value: 7, writable: true, configurable: true, enumerable: false
        });
        var sparseLength = sparse.unshift(1);
        length === 4 && values[0] === 1 && values[1] === 2 &&
        values[2] === 3 && values[3] === 4 &&
        Object.keys(values).join(',') === '1,2,3' &&
        sparseLength === 4098 && sparse[0] === 1 && sparse[4097] === 7 &&
        !(4096 in sparse) && Object.keys(sparse).join(',') === '0,4097';
    ";
    for mode in MODES {
        let mut vm = Vm::with_config(vm_config(mode, VmStorageLimits::unlimited()));
        ensure_true(&mut vm, SOURCE, "non-default and sparse fallback", mode)?;
        vm.storage_snapshot()
            .map_err(|error| format!("fallback in {mode:?}, before GC: {error}"))?;
        vm.collect_garbage()?;
        vm.storage_snapshot()
            .map_err(|error| format!("fallback in {mode:?}, after GC: {error}"))?;
    }
    Ok(())
}

#[test]
fn unshift_property_budgets_are_independent_for_vms_from_one_engine() -> TestResult {
    const SETUP: &str = "var values = [3]; values.unshift;";
    for mode in MODES {
        let initial_count = prepared_property_count(mode, SETUP)?;
        let limit = initial_count
            .checked_add(1)
            .ok_or("property limit overflow")?;
        let storage = VmStorageLimits::unlimited().with_max_count(PROPERTY_KIND, limit);
        let engine = Engine::with_config(EngineConfig::with_default_vm_config(vm_config(
            mode, storage,
        )));
        let mut first = engine.create_vm();
        let mut second = engine.create_vm();
        first.eval(SETUP)?;
        second.eval(SETUP)?;
        ensure_property_count(&first, initial_count, "first VM setup", mode)?;
        ensure_property_count(&second, initial_count, "second VM setup", mode)?;

        first.eval("values.unshift(1);")?;
        ensure_property_count(&first, limit, "first VM filled budget", mode)?;
        ensure_property_count(&second, initial_count, "second VM unused budget", mode)?;
        second.eval("values.unshift(2);")?;
        ensure_property_count(&second, limit, "second VM filled budget", mode)?;

        expect_property_limit(&mut first, "values.unshift(9);", "first VM", mode)?;
        expect_property_limit(&mut second, "values.unshift(8);", "second VM", mode)?;
        ensure_true(
            &mut first,
            "values.length === 2 && values[0] === 1 && values[1] === 3",
            "first VM unchanged after rejection",
            mode,
        )?;
        ensure_true(
            &mut second,
            "values.length === 2 && values[0] === 2 && values[1] === 3",
            "second VM unchanged after rejection",
            mode,
        )?;
        ensure_property_count(&first, limit, "first VM final budget", mode)?;
        ensure_property_count(&second, limit, "second VM final budget", mode)?;
    }
    Ok(())
}

fn check_transactional_unshift(
    label: &str,
    setup: &str,
    unchanged: &str,
    after_single_insert: &str,
) -> TestResult {
    for mode in MODES {
        let initial_count = prepared_property_count(mode, setup)?;
        let limit = initial_count
            .checked_add(1)
            .ok_or("property limit overflow")?;
        let storage = VmStorageLimits::unlimited().with_max_count(PROPERTY_KIND, limit);
        let mut vm = Vm::with_config(vm_config(mode, storage));
        vm.eval(setup)?;
        ensure_property_count(&vm, initial_count, label, mode)?;
        let before = vm.storage_snapshot()?;

        // One free slot must not permit a partial two-element insertion.
        expect_property_limit(&mut vm, "values.unshift(1, 2);", label, mode)?;
        let after = vm.storage_snapshot()?;
        if after.count(PROPERTY_KIND) != before.count(PROPERTY_KIND)
            || after.payload_bytes(PROPERTY_KIND) != before.payload_bytes(PROPERTY_KIND)
        {
            return Err(format!(
                "{label} in {mode:?}: rejected insertion changed property storage: \
                 before {before:?}, after {after:?}"
            )
            .into());
        }
        ensure_true(&mut vm, unchanged, label, mode)?;

        // A rejected reservation must leave the remaining slot available.
        vm.eval("values.unshift(1);")?;
        ensure_property_count(&vm, limit, label, mode)?;
        ensure_true(&mut vm, after_single_insert, label, mode)?;
        ensure_true(&mut vm, "values.unshift() === values.length", label, mode)?;
        ensure_property_count(&vm, limit, label, mode)?;

        // Removing the inserted property makes capacity reusable at the limit.
        ensure_true(&mut vm, "values.shift() === 1", label, mode)?;
        ensure_property_count(&vm, initial_count, label, mode)?;
        ensure_true(&mut vm, unchanged, label, mode)?;
        vm.eval("values.unshift(1);")?;
        ensure_property_count(&vm, limit, label, mode)?;
        ensure_true(&mut vm, after_single_insert, label, mode)?;
    }
    Ok(())
}

fn prepared_property_count(
    mode: OptimizationMode,
    setup: &str,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut probe = Vm::with_config(vm_config(mode, VmStorageLimits::unlimited()));
    probe.eval(setup)?;
    Ok(probe.storage_snapshot()?.count(PROPERTY_KIND))
}

fn vm_config(mode: OptimizationMode, storage: VmStorageLimits) -> VmConfig {
    VmConfig::with_limits(RuntimeLimits {
        storage,
        ..RuntimeLimits::default()
    })
    .with_optimization_mode(mode)
}

fn expect_property_limit(
    vm: &mut Vm,
    source: &str,
    label: &str,
    mode: OptimizationMode,
) -> TestResult {
    match vm.eval(source) {
        Err(error @ Error::ResourceLimit { .. })
            if error.to_string().contains("ObjectProperty") =>
        {
            Ok(())
        }
        actual => Err(format!(
            "{label} in {mode:?}: expected ObjectProperty resource limit for {source:?}, \
             got {actual:?}"
        )
        .into()),
    }
}

fn ensure_property_count(
    vm: &Vm,
    expected: usize,
    label: &str,
    mode: OptimizationMode,
) -> TestResult {
    let actual = vm
        .storage_snapshot()
        .map_err(|error| format!("{label} in {mode:?}: {error}"))?
        .count(PROPERTY_KIND);
    if actual != expected {
        return Err(format!(
            "{label} in {mode:?}: expected {expected} ObjectProperty records, got {actual}"
        )
        .into());
    }
    Ok(())
}

fn ensure_true(vm: &mut Vm, source: &str, label: &str, mode: OptimizationMode) -> TestResult {
    let actual = vm
        .eval(source)
        .map_err(|error| format!("{label} in {mode:?}, source {source:?}: {error}"))?;
    if actual != Value::Bool(true) {
        return Err(
            format!("{label} in {mode:?}: expected true for {source:?}, got {actual:?}").into(),
        );
    }
    Ok(())
}
