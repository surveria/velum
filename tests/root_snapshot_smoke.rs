use std::rc::Rc;

use parking_lot::Mutex;
use rs_quickjs::{Engine, Value, VmRootKind, VmRootSnapshot};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn snapshots_start_empty_and_use_stable_categories() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();

    let snapshot = vm.root_snapshot()?;
    ensure(
        snapshot.is_empty(),
        "expected a fresh VM to have no direct roots",
    )?;
    ensure_usize(snapshot.total(), 0, "fresh root total")?;
    ensure_usize(VmRootKind::all().len(), 14, "root kind count")?;

    let summed = VmRootKind::all().iter().try_fold(0_usize, |total, kind| {
        checked_root_sum(total, snapshot.count(*kind))
    })?;
    ensure_usize(summed, snapshot.total(), "fresh root category sum")
}

#[test]
fn snapshots_direct_roots_during_function_and_super_calls() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let function_snapshot = Rc::new(Mutex::new(None));
    let function_capture = Rc::clone(&function_snapshot);
    vm.register_host_function_typed("captureFunctionRoots", move |call| {
        *function_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;
    let super_snapshot = Rc::new(Mutex::new(None));
    let super_capture = Rc::clone(&super_snapshot);
    vm.register_host_function_typed("captureSuperRoots", move |call| {
        *super_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;

    vm.eval(
        r"
        function makeProbe() {
            let captured = 40;
            return function probe(argument) {
                let local = 2;
                return captureFunctionRoots() + captured + local + argument;
            };
        }
        var probe = makeProbe();
        function outerProbe() {
            let outerLocal = 1;
            return probe(outerLocal - 1);
        }
        outerProbe();

        class Base {
            probe() {
                return 42;
            }
        }
        class Child extends Base {
            probe() {
                captureSuperRoots();
                return super.probe();
            }
        }
        new Child().probe();
        ",
    )?;

    let active = copied_snapshot(&function_snapshot, "function root snapshot")?;
    ensure_positive(active.count(VmRootKind::LocalBinding), "active local roots")?;
    ensure_positive(
        active.count(VmRootKind::CapturedBinding),
        "active captured roots",
    )?;
    ensure_positive(active.count(VmRootKind::ActiveThis), "active this roots")?;
    ensure_at_least(
        active.count(VmRootKind::ActiveThis),
        2,
        "nested activation this roots",
    )?;
    ensure_positive(
        active.count(VmRootKind::ActiveNewTarget),
        "active new.target roots",
    )?;
    ensure_positive(
        active.count(VmRootKind::BytecodeFrame),
        "active bytecode function roots",
    )?;
    ensure_at_least(
        active.count(VmRootKind::ActiveNewTarget),
        2,
        "nested activation new.target roots",
    )?;

    let with_super = copied_snapshot(&super_snapshot, "super root snapshot")?;
    ensure_positive(
        with_super.count(VmRootKind::ActiveSuper),
        "active super roots",
    )?;

    let settled = vm.root_snapshot()?;
    ensure_settled_activation_roots(&settled)?;
    ensure_positive(
        settled.count(VmRootKind::GlobalBinding),
        "settled global roots",
    )?;
    ensure_snapshot_sum(settled)
}

#[test]
fn snapshots_pending_structured_control_values() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let control_snapshot = Rc::new(Mutex::new(None));
    let control_capture = Rc::clone(&control_snapshot);
    vm.register_host_function_typed("captureControlRoots", move |call| {
        *control_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;

    vm.eval(
        r"
        let marker = { durable: true };
        try {
            throw marker;
        } catch (error) {
            captureControlRoots();
        }
        ",
    )?;

    let active = copied_snapshot(&control_snapshot, "structured control root snapshot")?;
    ensure_positive(
        active.count(VmRootKind::TransientTemporary),
        "running structured control roots",
    )?;
    let settled = vm.root_snapshot()?;
    ensure_usize(
        settled.count(VmRootKind::BytecodeFrame),
        0,
        "settled structured control roots",
    )?;
    ensure_snapshot_sum(settled)
}

fn ensure_settled_activation_roots(settled: &VmRootSnapshot) -> TestResult {
    ensure_usize(
        settled.count(VmRootKind::LocalBinding),
        0,
        "settled local roots",
    )?;
    ensure_usize(
        settled.count(VmRootKind::CapturedBinding),
        0,
        "settled captured roots",
    )?;
    ensure_usize(
        settled.count(VmRootKind::ActiveThis),
        0,
        "settled this roots",
    )?;
    ensure_usize(
        settled.count(VmRootKind::ActiveNewTarget),
        0,
        "settled new.target roots",
    )?;
    ensure_usize(
        settled.count(VmRootKind::ActiveSuper),
        0,
        "settled super roots",
    )?;
    ensure_usize(
        settled.count(VmRootKind::BytecodeFrame),
        0,
        "settled bytecode frame roots",
    )
}

#[test]
fn snapshots_builtin_anchors_and_queued_promise_jobs() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let queued_snapshot = Rc::new(Mutex::new(None));
    let queued_capture = Rc::clone(&queued_snapshot);
    vm.register_host_function_typed("captureQueuedRoots", move |call| {
        *queued_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;

    vm.eval(
        r"
        Promise.resolve(1).then(function firstReaction() {
            captureQueuedRoots();
        });
        Promise.resolve(2).then(function secondReaction() {
            return 0;
        });
        ",
    )?;

    let queued = copied_snapshot(&queued_snapshot, "queued Promise root snapshot")?;
    ensure_positive(
        queued.count(VmRootKind::QueuedJob),
        "queued Promise job roots",
    )?;
    ensure_positive(queued.count(VmRootKind::BuiltinBinding), "builtin roots")?;
    ensure_positive(
        queued.count(VmRootKind::RuntimeAnchor),
        "runtime anchor roots",
    )?;
    ensure_snapshot_sum(queued)?;

    let settled = vm.root_snapshot()?;
    ensure_usize(
        settled.count(VmRootKind::QueuedJob),
        0,
        "settled queued job roots",
    )?;
    ensure_positive(
        settled.count(VmRootKind::BuiltinBinding),
        "settled builtin roots",
    )?;
    ensure_positive(
        settled.count(VmRootKind::RuntimeAnchor),
        "settled runtime anchor roots",
    )?;
    ensure_snapshot_sum(settled)
}

fn copied_snapshot(
    source: &Mutex<Option<VmRootSnapshot>>,
    label: &str,
) -> Result<VmRootSnapshot, Box<dyn std::error::Error>> {
    let snapshot = *source.lock();
    snapshot.ok_or_else(|| format!("expected {label} to be captured").into())
}

fn ensure_at_least(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual >= minimum {
        return Ok(());
    }
    Err(format!("expected {label} to be at least {minimum}, got {actual}").into())
}

fn ensure_snapshot_sum(snapshot: VmRootSnapshot) -> TestResult {
    let summed = VmRootKind::all().iter().try_fold(0_usize, |total, kind| {
        checked_root_sum(total, snapshot.count(*kind))
    })?;
    ensure_usize(summed, snapshot.total(), "root category sum")
}

fn checked_root_sum(total: usize, count: usize) -> Result<usize, Box<dyn std::error::Error>> {
    total
        .checked_add(count)
        .ok_or_else(|| "root snapshot sum overflowed".into())
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.into())
}

fn ensure_positive(actual: usize, label: &str) -> TestResult {
    if actual > 0 {
        return Ok(());
    }
    Err(format!("expected {label} to be positive, got {actual}").into())
}

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected}, got {actual}").into())
}

#[test]
fn vm_snapshot_does_not_change_evaluation_result() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.root_snapshot()?;
    let value = vm.eval("40 + 2")?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected 42 after snapshot, got {value:?}").into())
}
