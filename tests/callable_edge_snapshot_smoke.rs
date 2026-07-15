use velum::{Engine, Value, VmCallableEdgeKind, VmCallableEdgeSnapshot, VmRootKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn callable_edge_snapshots_start_empty_and_sum_stable_categories() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();

    let snapshot = vm.callable_edge_snapshot()?;
    ensure(
        snapshot.is_empty(),
        "expected a fresh VM to have no callable edges",
    )?;
    ensure_usize(snapshot.total(), 0, "fresh callable edge total")?;
    ensure_usize(
        VmCallableEdgeKind::all().len(),
        7,
        "callable edge kind count",
    )?;
    ensure_snapshot_sum(snapshot)
}

#[test]
fn snapshots_host_function_properties() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_host_function_typed("hostEdge", |_call| Ok(()))?;
    vm.eval("hostEdge.metadata = { answer: 42 }")?;

    let snapshot = vm.callable_edge_snapshot()?;
    ensure_positive(
        snapshot.count(VmCallableEdgeKind::HostFunctionProperty),
        "host function property edges",
    )?;
    ensure_snapshot_sum(snapshot)
}

#[test]
fn snapshots_javascript_function_upvalues_properties_and_internal_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.eval(
        r"
        function makeClosure() {
            let captured = 40;
            return function closure(addend) {
                return captured + addend;
            };
        }
        var closure = makeClosure();
        closure.extra = { answer: 42 };

        function makeArrow() {
            return () => new.target;
        }
        var lexicalNewTarget = makeArrow();

        class Base {}
        class Child extends Base {
            field = 42;
            method() {
                return super.constructor;
            }
        }
        var child = new Child();
        ",
    )?;

    let snapshot = vm.callable_edge_snapshot()?;
    ensure_positive(
        snapshot.count(VmCallableEdgeKind::JavaScriptFunctionUpvalue),
        "JavaScript function upvalue edges",
    )?;
    ensure_positive(
        snapshot.count(VmCallableEdgeKind::JavaScriptFunctionProperty),
        "JavaScript function property edges",
    )?;
    ensure_positive(
        snapshot.count(VmCallableEdgeKind::JavaScriptFunctionInternal),
        "JavaScript function internal edges",
    )?;
    ensure_snapshot_sum(snapshot)?;

    let value = vm.eval("closure(2)")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn snapshots_native_payload_ids_and_bound_function_values() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.eval(
        r"
        function target(left, right) {
            return this.base + left + right;
        }
        var bound = target.bind({ base: 39 }, 1);
        var iterator = new Map([[1, 2]]).entries();
        var deferred = new Promise(function executor(resolve) {
            resolve(42);
        });
        var revocable = Proxy.revocable({}, {});
        ",
    )?;

    let snapshot = vm.callable_edge_snapshot()?;
    ensure_positive(
        snapshot.count(VmCallableEdgeKind::NativeFunctionProperty),
        "native function property edges",
    )?;
    ensure_at_least(
        snapshot.count(VmCallableEdgeKind::NativeFunctionInternal),
        4,
        "native function internal edges",
    )?;
    ensure_at_least(
        snapshot.count(VmCallableEdgeKind::BoundFunctionInternal),
        3,
        "bound function internal edges",
    )?;
    ensure_snapshot_sum(snapshot)?;

    let value = vm.eval("bound(2)")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn registered_native_functions_are_runtime_anchor_roots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let initial = vm.root_snapshot()?;
    ensure_usize(
        initial.count(VmRootKind::RuntimeAnchor),
        0,
        "initial runtime anchor roots",
    )?;

    vm.eval("Math.max(40, 2)")?;
    let installed = vm.root_snapshot()?;
    ensure_positive(
        installed.count(VmRootKind::RuntimeAnchor),
        "native registry runtime anchor roots",
    )
}

fn ensure_snapshot_sum(snapshot: VmCallableEdgeSnapshot) -> TestResult {
    let summed = VmCallableEdgeKind::all()
        .iter()
        .try_fold(0_usize, |total, kind| {
            checked_sum(total, snapshot.count(*kind))
        })?;
    ensure_usize(summed, snapshot.total(), "callable edge category sum")
}

fn checked_sum(total: usize, count: usize) -> Result<usize, Box<dyn std::error::Error>> {
    total
        .checked_add(count)
        .ok_or_else(|| "callable edge snapshot sum overflowed".into())
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
