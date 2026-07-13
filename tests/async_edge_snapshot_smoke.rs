use rs_quickjs::{Engine, Value, VmAsyncEdgeKind, VmAsyncEdgeSnapshot, VmAsyncEdgeStrength};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn async_edge_snapshots_start_empty_and_classify_stable_categories() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();

    let snapshot = vm.async_edge_snapshot()?;
    ensure(
        snapshot.is_empty(),
        "expected a fresh VM to have no asynchronous edges",
    )?;
    ensure_usize(snapshot.total(), 0, "fresh asynchronous edge total")?;
    ensure_usize(
        VmAsyncEdgeKind::all().len(),
        15,
        "asynchronous edge kind count",
    )?;
    ensure_usize(
        VmAsyncEdgeStrength::all().len(),
        3,
        "asynchronous edge strength count",
    )?;
    ensure_strength(VmAsyncEdgeKind::PromiseState, VmAsyncEdgeStrength::Strong)?;
    ensure_strength(
        VmAsyncEdgeKind::WeakCollectionKey,
        VmAsyncEdgeStrength::Weak,
    )?;
    ensure_strength(
        VmAsyncEdgeKind::WeakCollectionEphemeron,
        VmAsyncEdgeStrength::Ephemeron,
    )?;
    ensure_strength(
        VmAsyncEdgeKind::FinalizationRegistryHeldValue,
        VmAsyncEdgeStrength::Strong,
    )?;
    ensure_strength(VmAsyncEdgeKind::WeakRefTarget, VmAsyncEdgeStrength::Weak)?;
    ensure_snapshot_sums(snapshot)
}

#[test]
fn snapshots_promise_associations_states_and_pending_reactions() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.eval(
        r"
        var pending = new Promise(function executor() {});
        var chained = pending.then(
            function fulfilled(value) { return value; },
            function rejected(reason) { return reason; }
        );
        var settled = Promise.resolve({ answer: 42 });
        ",
    )?;

    let snapshot = vm.async_edge_snapshot()?;
    ensure_at_least(
        snapshot.count(VmAsyncEdgeKind::PromiseObjectAssociation),
        3,
        "Promise object associations",
    )?;
    ensure_positive(
        snapshot.count(VmAsyncEdgeKind::PromiseState),
        "settled Promise state edges",
    )?;
    ensure_at_least(
        snapshot.count(VmAsyncEdgeKind::PromiseReaction),
        3,
        "pending Promise reaction edges",
    )?;
    ensure_snapshot_sums(snapshot)?;

    let value = vm.eval("settled instanceof Promise && chained instanceof Promise")?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn snapshots_map_set_entries_associations_and_iterator_items() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.eval(
        r"
        var mapKey = { key: 1 };
        var mapValue = { value: 2 };
        var map = new Map([[mapKey, mapValue]]);
        var setValue = { value: 3 };
        var set = new Set([setValue]);
        var iterator = map.entries();
        ",
    )?;

    let snapshot = vm.async_edge_snapshot()?;
    ensure_at_least(
        snapshot.count(VmAsyncEdgeKind::CollectionObjectAssociation),
        2,
        "collection object associations",
    )?;
    ensure_at_least(
        snapshot.count(VmAsyncEdgeKind::CollectionEntry),
        4,
        "Map and Set physical entry slots",
    )?;
    ensure_positive(
        snapshot.count(VmAsyncEdgeKind::IteratorItem),
        "collection iterator item edges",
    )?;
    ensure_snapshot_sums(snapshot)?;

    let value = vm.eval("map.get(mapKey) === mapValue && set.has(setValue)")?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn snapshots_weak_set_keys_and_weak_map_ephemerons_without_strong_entries() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.eval(
        r"
        var weakMapKey = { key: 1 };
        var weakMapValue = { value: 2 };
        var weakMap = new WeakMap([[weakMapKey, weakMapValue]]);
        var weakSetKey = { key: 3 };
        var weakSet = new WeakSet([weakSetKey]);
        ",
    )?;

    let snapshot = vm.async_edge_snapshot()?;
    ensure_usize(
        snapshot.count(VmAsyncEdgeKind::CollectionEntry),
        0,
        "ordinary strong collection entries",
    )?;
    ensure_usize(
        snapshot.count(VmAsyncEdgeKind::WeakCollectionKey),
        1,
        "WeakSet weak keys",
    )?;
    ensure_usize(
        snapshot.count(VmAsyncEdgeKind::WeakCollectionEphemeron),
        1,
        "WeakMap ephemeron pairs",
    )?;
    ensure_usize(
        snapshot.count_by_strength(VmAsyncEdgeStrength::Weak),
        1,
        "weak trace records",
    )?;
    ensure_usize(
        snapshot.count_by_strength(VmAsyncEdgeStrength::Ephemeron),
        1,
        "ephemeron trace records",
    )?;
    ensure_snapshot_sums(snapshot)?;

    let value = vm.eval("weakMap.get(weakMapKey) === weakMapValue && weakSet.has(weakSetKey)")?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn snapshots_finalization_registry_and_weak_ref_edge_strengths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.eval(
        r"
        var cleanup = function cleanup() {};
        var target = {};
        var heldValue = {};
        var token = {};
        var registry = new FinalizationRegistry(cleanup);
        registry.register(target, heldValue, token);
        var reference = new WeakRef(target);
        ",
    )?;

    let snapshot = vm.async_edge_snapshot()?;
    ensure_usize(
        snapshot.count(VmAsyncEdgeKind::FinalizationRegistryCleanupCallback),
        1,
        "FinalizationRegistry cleanup callback",
    )?;
    ensure_usize(
        snapshot.count(VmAsyncEdgeKind::FinalizationRegistryHeldValue),
        1,
        "FinalizationRegistry held value",
    )?;
    ensure_usize(
        snapshot.count(VmAsyncEdgeKind::FinalizationRegistryTarget),
        1,
        "FinalizationRegistry weak target",
    )?;
    ensure_usize(
        snapshot.count(VmAsyncEdgeKind::FinalizationRegistryUnregisterToken),
        1,
        "FinalizationRegistry weak unregister token",
    )?;
    ensure_usize(
        snapshot.count(VmAsyncEdgeKind::WeakRefTarget),
        1,
        "WeakRef weak target",
    )?;
    ensure_snapshot_sums(snapshot)?;

    let value = vm.eval("reference.deref() === target && registry.unregister(token)")?;
    ensure_value(&value, &Value::Bool(true))
}

fn ensure_snapshot_sums(snapshot: VmAsyncEdgeSnapshot) -> TestResult {
    let kinds = VmAsyncEdgeKind::all()
        .iter()
        .try_fold(0_usize, |total, kind| {
            checked_sum(total, snapshot.count(*kind))
        })?;
    ensure_usize(kinds, snapshot.total(), "asynchronous edge category sum")?;

    let strengths = VmAsyncEdgeStrength::all()
        .iter()
        .try_fold(0_usize, |total, strength| {
            checked_sum(total, snapshot.count_by_strength(*strength))
        })?;
    ensure_usize(
        strengths,
        snapshot.total(),
        "asynchronous edge strength sum",
    )
}

fn checked_sum(total: usize, count: usize) -> Result<usize, Box<dyn std::error::Error>> {
    total
        .checked_add(count)
        .ok_or_else(|| "asynchronous edge snapshot sum overflowed".into())
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.into())
}

fn ensure_strength(kind: VmAsyncEdgeKind, expected: VmAsyncEdgeStrength) -> TestResult {
    let actual = kind.strength();
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {kind:?} strength {expected:?}, got {actual:?}").into())
}

fn ensure_positive(actual: usize, label: &str) -> TestResult {
    if actual > 0 {
        return Ok(());
    }
    Err(format!("expected {label} to be positive, got {actual}").into())
}

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }
    Err(format!("expected {label} >= {expected}, got {actual}").into())
}

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected}, got {actual}").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
