use velum::{Engine, Value, Vm, VmGarbageCollectionReport, VmGcKind, VmStorageKind};

type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
fn reverse_ephemeron_chain_reaches_fixed_point_and_releases_all_entries() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        var reverseWeak = new WeakMap();
        var reverseRoot = {};
        (function seedReverseChain() {
            const first = {};
            const second = {};
            const third = {};
            reverseWeak.set(third, { answer: 42 });
            reverseWeak.set(second, third);
            reverseWeak.set(first, second);
            reverseWeak.set(reverseRoot, first);
        })();
        0;
        ",
    )?;

    collect_checked(&mut vm, 4, 4, 0)?;
    ensure_value(
        &vm.eval(
            "reverseWeak.get(reverseWeak.get(reverseWeak.get(reverseWeak.get(reverseRoot)))).answer",
        )?,
        &Value::Number(42.0),
        "reverse chain terminal value after collection",
    )?;

    vm.eval("reverseRoot = null; 0;")?;
    let report = collect_checked(&mut vm, 4, 0, 4)?;
    ensure_at_least(
        report.reclaimed(VmGcKind::Object),
        5,
        "released chain objects",
    )?;
    collect_checked(&mut vm, 0, 0, 0)?;
    Ok(())
}

#[test]
fn dead_key_cycle_does_not_become_a_strong_root() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        var cyclicWeak = new WeakMap();
        (function seedDeadCycle() {
            const left = {};
            const right = {};
            left.peer = right;
            right.peer = left;
            cyclicWeak.set(left, right);
            cyclicWeak.set(right, left);
        })();
        0;
        ",
    )?;

    ensure_at_least(
        vm.heap_reachability_snapshot()?
            .unreachable(VmGcKind::Object),
        2,
        "unreachable cycle objects before collection",
    )?;
    let report = collect_checked(&mut vm, 2, 0, 2)?;
    ensure_at_least(
        report.reclaimed(VmGcKind::Object),
        2,
        "reclaimed cycle objects",
    )?;
    collect_checked(&mut vm, 0, 0, 0)?;
    Ok(())
}

#[test]
fn mixed_callable_and_symbol_chain_preserves_weak_identity_categories() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_host_function_typed("weakHostKey", |_call| Ok(10.0))?;
    vm.eval(
        r"
        var mixedWeak = new WeakMap();
        var mixedRoot = {};
        (function seedMixedChain() {
            const first = function firstWeakKey() {};
            const symbol = Symbol();
            const second = function secondWeakKey() {};
            mixedWeak.set(second, { answer: 42 });
            mixedWeak.set(symbol, second);
            mixedWeak.set(first, symbol);
            mixedWeak.set(mixedRoot, first);
            mixedWeak.set(Array, 9);
            mixedWeak.set(weakHostKey, 10);
        })();
        0;
        ",
    )?;

    collect_checked(&mut vm, 6, 6, 0)?;
    ensure_value(
        &vm.eval(
            "mixedWeak.get(mixedWeak.get(mixedWeak.get(mixedWeak.get(mixedRoot)))).answer === 42
                && mixedWeak.get(Array) === 9 && mixedWeak.get(weakHostKey) === 10",
        )?,
        &Value::Bool(true),
        "callable and Symbol chain values after collection",
    )?;

    vm.eval("mixedRoot = null; 0;")?;
    let report = collect_checked(&mut vm, 6, 2, 4)?;
    ensure_at_least(
        report.reclaimed(VmGcKind::JavaScriptFunction),
        2,
        "reclaimed callable chain keys",
    )?;
    ensure_at_least(
        report.reclaimed(VmGcKind::Symbol),
        1,
        "reclaimed Symbol chain key",
    )?;
    ensure_value(
        &vm.eval("mixedWeak.get(Array) === 9 && mixedWeak.get(weakHostKey) === 10")?,
        &Value::Bool(true),
        "independently rooted native and host keys after chain release",
    )?;
    Ok(())
}

#[test]
fn newly_discovered_weak_map_contributes_ephemerons_on_the_next_pass() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        var outerWeak = new WeakMap();
        var outerRoot = {};
        (function seedNestedWeakMap() {
            const inner = new WeakMap();
            inner.set(outerRoot, { answer: 42 });
            outerWeak.set(outerRoot, inner);
        })();
        0;
        ",
    )?;

    collect_checked(&mut vm, 2, 2, 0)?;
    ensure_value(
        &vm.eval("outerWeak.get(outerRoot).get(outerRoot).answer")?,
        &Value::Number(42.0),
        "value reached through a newly discovered WeakMap",
    )?;

    vm.eval("outerRoot = null; 0;")?;
    // The live outer map sweeps one entry; the dead inner map releases its whole store.
    let report = collect_checked(&mut vm, 2, 0, 1)?;
    ensure_count(
        report.reclaimed(VmGcKind::Collection),
        1,
        "reclaimed inner collection",
    )?;
    ensure_at_least(
        report.reclaimed(VmGcKind::Object),
        3,
        "reclaimed nested graph objects",
    )?;
    Ok(())
}

fn collect_checked(
    vm: &mut Vm,
    entries_before: usize,
    entries_after: usize,
    removed_weak_entries: usize,
) -> TestResult<VmGarbageCollectionReport> {
    let before = vm.storage_snapshot()?;
    ensure_count(
        before.count(VmStorageKind::CollectionEntry),
        entries_before,
        "collection entries before collection",
    )?;
    let report = vm.collect_garbage()?;
    let after = vm.storage_snapshot()?;
    ensure_count(
        after.count(VmStorageKind::CollectionEntry),
        entries_after,
        "collection entries after collection",
    )?;
    ensure_count(
        report.weak_entries_removed(),
        removed_weak_entries,
        "swept weak entries",
    )?;
    for (storage, kind) in [
        (VmStorageKind::Object, VmGcKind::Object),
        (
            VmStorageKind::JavaScriptFunction,
            VmGcKind::JavaScriptFunction,
        ),
        (VmStorageKind::Symbol, VmGcKind::Symbol),
        (VmStorageKind::Collection, VmGcKind::Collection),
    ] {
        let released = before
            .count(storage)
            .checked_sub(after.count(storage))
            .ok_or_else(|| format!("{storage:?} storage increased during collection"))?;
        ensure_count(
            report.reclaimed(kind),
            released,
            "reclaimed records match storage",
        )?;
    }
    ensure_count(
        after.count(VmStorageKind::TransientRoot),
        0,
        "settled transient roots",
    )?;
    Ok(report)
}

fn ensure_count(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual != expected {
        return Err(format!("{label}: expected {expected}, got {actual}").into());
    }
    Ok(())
}

fn ensure_at_least(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual < minimum {
        return Err(format!("{label}: expected at least {minimum}, got {actual}").into());
    }
    Ok(())
}

fn ensure_value(actual: &Value, expected: &Value, label: &str) -> TestResult {
    if actual != expected {
        return Err(format!("{label}: expected {expected:?}, got {actual:?}").into());
    }
    Ok(())
}
