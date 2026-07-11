use rs_quickjs::{
    Engine, EngineConfig, RuntimeLimits, VmAsyncEdgeKind, VmConfig, VmGcKind, VmStorageKind,
    VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn traces_and_reclaims_suspended_generator_state() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        var suspendedGenerator = (function* values() {
            const retainedByFrame = { answer: 42 };
            yield 1;
            return retainedByFrame.answer;
        })();
        suspendedGenerator.next();
        ",
    )?;

    let edges = vm.async_edge_snapshot()?;
    ensure_positive(
        edges.count(VmAsyncEdgeKind::GeneratorObjectAssociation),
        "generator object association edges",
    )?;
    ensure_positive(
        edges.count(VmAsyncEdgeKind::GeneratorState),
        "suspended generator state edges",
    )?;

    let before = vm.heap_reachability_snapshot()?;
    ensure_positive(
        before.reachable(VmGcKind::Generator),
        "reachable generator records",
    )?;
    vm.collect_garbage()?;
    let value = vm.eval("suspendedGenerator.next().value")?;
    ensure(
        value.to_string() == "42",
        "collection lost a value retained only by a suspended generator frame",
    )?;

    vm.eval("suspendedGenerator = null")?;
    let released = vm.heap_reachability_snapshot()?;
    ensure_positive(
        released.unreachable(VmGcKind::Generator),
        "unreachable generator records",
    )?;
    let report = vm.collect_garbage()?;
    ensure_positive(
        report.reclaimed(VmGcKind::Generator),
        "reclaimed generator records",
    )?;
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn traces_nested_yield_delegate_across_collection() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        function* outerGenerator() {
            function* innerGenerator() {
                const received = yield 1;
                return received + 1;
            }
            return yield* innerGenerator();
        }
        var delegatedGenerator = outerGenerator();
        delegatedGenerator.next();
        ",
    )?;

    vm.collect_garbage()?;
    let value = vm.eval("delegatedGenerator.next(41).value")?;
    ensure(
        value.to_string() == "42",
        "collection lost a nested generator retained by yield delegation",
    )?;
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn marks_strong_roots_and_reports_unreachable_records() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let retained = vm.eval_retained("({ retained: { value: 42 } })")?;
    vm.eval(
        r"
        var rooted = { child: { value: 7 } };
        function capture() {
            return rooted.child;
        }
        (function createGarbage() {
            let discarded = { child: { value: 1 } };
            return discarded.value;
        })();
        42
        ",
    )?;

    let snapshot = vm.heap_reachability_snapshot()?;
    ensure_positive(
        snapshot.reachable(VmGcKind::Object),
        "reachable object count",
    )?;
    ensure_at_least(
        snapshot.unreachable(VmGcKind::Object),
        2,
        "unreachable object count",
    )?;
    ensure_positive(
        snapshot.reachable(VmGcKind::JavaScriptFunction),
        "reachable function count",
    )?;

    drop(retained);
    let released = vm.heap_reachability_snapshot()?;
    ensure(
        released.unreachable(VmGcKind::Object) > snapshot.unreachable(VmGcKind::Object),
        "releasing a retained handle did not expose its object graph",
    )?;
    let report = vm.collect_garbage()?;
    ensure_at_least(
        report.reclaimed(VmGcKind::Object),
        released.unreachable(VmGcKind::Object),
        "reclaimed object records",
    )?;
    ensure_positive(report.total_reclaimed(), "total reclaimed records")?;
    ensure_usize(
        vm.heap_reachability_snapshot()?
            .unreachable(VmGcKind::Object),
        0,
        "unreachable objects after collection",
    )?;
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn resolves_weak_map_ephemerons_without_marking_dead_keys() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r#"
        var liveKey = { name: "live" };
        var liveValue = { answer: 42 };
        var weak = new WeakMap();
        weak.set(liveKey, liveValue);
        (function addDeadEntry() {
            let deadKey = { name: "dead" };
            let deadValue = { answer: 1 };
            weak.set(deadKey, deadValue);
        })();
        42
        "#,
    )?;

    let snapshot = vm.heap_reachability_snapshot()?;
    ensure_positive(
        snapshot.reachable(VmGcKind::Collection),
        "reachable WeakMap storage",
    )?;
    ensure_at_least(
        snapshot.unreachable(VmGcKind::Object),
        2,
        "dead WeakMap key and value",
    )?;
    ensure(
        snapshot.reachable(VmGcKind::Object) >= 3,
        "live WeakMap key did not retain its ephemeron value",
    )?;

    let report = vm.collect_garbage()?;
    ensure_usize(report.weak_entries_removed(), 1, "removed weak entries")?;
    let result = vm.eval("weak.has(liveKey) && weak.get(liveKey) === liveValue ? 42 : 0")?;
    ensure(
        result.to_string() == "42",
        "live WeakMap ephemeron did not survive collection",
    )?;
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn distinguishes_registered_and_unreachable_symbols() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r#"
        Symbol.for("registered");
        (function createDeadSymbol() {
            let weak = new WeakSet();
            weak.add(Symbol("dead"));
        })();
        42
        "#,
    )?;

    let snapshot = vm.heap_reachability_snapshot()?;
    ensure_positive(
        snapshot.reachable(VmGcKind::Symbol),
        "registered symbol root",
    )?;
    ensure_positive(
        snapshot.unreachable(VmGcKind::Symbol),
        "unreachable weak symbol",
    )?;
    let report = vm.collect_garbage()?;
    ensure_at_least(
        report.reclaimed(VmGcKind::Symbol),
        snapshot.unreachable(VmGcKind::Symbol),
        "reclaimed Symbol records",
    )?;
    ensure_usize(
        vm.heap_reachability_snapshot()?
            .unreachable(VmGcKind::Symbol),
        0,
        "unreachable Symbols after collection",
    )?;
    let registered =
        vm.eval(r#"Symbol.keyFor(Symbol.for("registered")) === "registered" ? 42 : 0"#)?;
    ensure(
        registered.to_string() == "42",
        "registered Symbol did not survive collection",
    )
}

#[test]
fn reuses_collected_object_slots_under_a_hard_limit() -> TestResult {
    let storage = VmStorageLimits::unlimited().with_max_count(VmStorageKind::Object, 32);
    let limits = RuntimeLimits {
        storage,
        ..RuntimeLimits::default()
    };
    let config = EngineConfig::with_default_vm_config(VmConfig::with_limits(limits));
    let engine = Engine::with_config(config);
    let mut vm = engine.create_vm();

    for _ in 0..100 {
        vm.eval("({ payload: { answer: 42 } });")?;
        vm.collect_garbage()?;
    }
    let snapshot = vm.storage_snapshot()?;
    ensure(
        snapshot.count(VmStorageKind::Object) < 32,
        "object records were not reclaimed below the hard limit",
    )
}

#[test]
fn preserves_pending_jobs_and_suspended_async_owners() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        var resolveLater;
        var observed = 0;
        var pending = new Promise(function(resolve) {
            resolveLater = resolve;
        });
        async function waitForValue() {
            observed = await pending;
        }
        waitForValue();
        42
        ",
    )?;

    let before = vm.heap_reachability_snapshot()?;
    ensure_positive(
        before.reachable(VmGcKind::Promise),
        "reachable pending Promise",
    )?;
    vm.collect_garbage()?;
    vm.eval("resolveLater(42); 0")?;
    let observed = vm.eval("observed")?;
    ensure(
        observed.to_string() == "42",
        "suspended async activation did not resume after collection",
    )?;
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn reclaims_suspended_owners_after_embedder_cancellation() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        var resolveLater;
        var resumed = false;
        async function task() {
            await new Promise(function(resolve) { resolveLater = resolve; });
            resumed = true;
        }
        task();
        42
        ",
    )?;

    let before = vm.heap_reachability_snapshot()?;
    ensure_positive(
        before.reachable(VmGcKind::Promise),
        "reachable suspended Promise owner",
    )?;
    ensure_positive(vm.cancel_jobs()?, "cancelled suspended reactions")?;
    let cancelled = vm.heap_reachability_snapshot()?;
    ensure(
        cancelled.unreachable(VmGcKind::Promise) > before.unreachable(VmGcKind::Promise),
        "cancellation did not expose the suspended result Promise",
    )?;
    let report = vm.collect_garbage()?;
    ensure_positive(
        report.reclaimed(VmGcKind::Promise),
        "reclaimed cancelled Promise owners",
    )?;
    vm.eval("resolveLater(42); 0")?;
    ensure(
        vm.eval("resumed")?.to_string() == "false",
        "cancelled async continuation resumed after collection",
    )?;
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn reclaims_heap_strings_and_preserves_rooted_text() -> TestResult {
    let storage = VmStorageLimits::unlimited().with_max_count(VmStorageKind::HeapString, 64);
    let limits = RuntimeLimits {
        storage,
        ..RuntimeLimits::default()
    };
    let config = EngineConfig::with_default_vm_config(VmConfig::with_limits(limits));
    let engine = Engine::with_config(config);
    let mut vm = engine.create_vm();
    vm.eval("var keptText = 'kept-text'; 'dead-' + 'text';")?;
    let before = vm.heap_reachability_snapshot()?;
    ensure_positive(
        before.reachable(VmGcKind::HeapString),
        "reachable heap string",
    )?;
    ensure_positive(
        before.unreachable(VmGcKind::HeapString),
        "unreachable heap string",
    )?;
    let report = vm.collect_garbage()?;
    ensure_positive(
        report.reclaimed(VmGcKind::HeapString),
        "reclaimed heap strings",
    )?;

    for index in 0..100 {
        vm.eval(&format!("'temporary-{index}' + '-value';"))?;
        vm.collect_garbage()?;
    }
    let kept = vm.eval("keptText")?;
    ensure(
        kept.to_string() == "kept-text",
        "rooted heap string did not survive collection",
    )?;
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn invalidates_callable_caches_before_reusing_native_ids() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        var target = (function() { return 1; }).bind(null);
        function invokeTarget() {
            return target();
        }
        invokeTarget();
        ",
    )?;
    vm.eval("target = null; 0")?;
    let report = vm.collect_garbage()?;
    ensure_positive(
        report.reclaimed(VmGcKind::NativeFunction),
        "reclaimed ephemeral native function",
    )?;
    ensure_positive(
        report.reclaimed(VmGcKind::BoundFunction),
        "reclaimed bound function payload",
    )?;

    vm.eval(
        r"
        var iterator = new Map([[1, 42]]).values();
        target = iterator.next;
        0
        ",
    )?;
    let result = vm.eval("invokeTarget().value")?;
    ensure(
        result.to_string() == "42",
        "call cache dispatched a reused native id through its old kind",
    )
}

#[test]
fn collection_remains_isolated_between_vms() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let mut second = engine.create_vm();
    first.eval("var owned = { vm: 'first' }; ({ garbage: true });")?;
    second.eval("var owned = { vm: 'second' }; ({ garbage: true });")?;

    let second_before = second.storage_snapshot()?;
    first.collect_garbage()?;
    let second_after = second.storage_snapshot()?;
    ensure(
        second_after == second_before,
        "collecting one VM changed another VM's storage",
    )?;
    ensure(
        second.eval("owned.vm")?.to_string() == "second",
        "collecting one VM changed another VM's rooted value",
    )?;
    ensure(
        first.eval("owned.vm")?.to_string() == "first",
        "collection removed the first VM's rooted value",
    )
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.to_owned().into())
}

fn ensure_positive(value: usize, label: &str) -> TestResult {
    ensure(
        value > 0,
        &format!("expected positive {label}, got {value}"),
    )
}

fn ensure_at_least(value: usize, minimum: usize, label: &str) -> TestResult {
    ensure(
        value >= minimum,
        &format!("expected {label} >= {minimum}, got {value}"),
    )
}

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    ensure(
        actual == expected,
        &format!("expected {label} {expected}, got {actual}"),
    )
}
