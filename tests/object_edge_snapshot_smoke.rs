use velum::{Engine, Value, VmObjectEdgeKind, VmObjectEdgeSnapshot, VmRootKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn object_edge_snapshots_start_empty_and_sum_stable_categories() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();

    let snapshot = vm.object_edge_snapshot()?;
    ensure(
        snapshot.is_empty(),
        "expected a fresh VM to have no object edges",
    )?;
    ensure_usize(snapshot.total(), 0, "fresh object edge total")?;
    ensure_usize(VmObjectEdgeKind::all().len(), 3, "object edge kind count")?;
    ensure_snapshot_sum(snapshot)
}

#[test]
fn snapshots_named_accessor_dense_sparse_and_prototype_edges() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.eval(
        r#"
        var symbolKey = Symbol("edge");
        var prototype = { inherited: 40 };
        var object = Object.create(prototype);
        object.data = { value: 1 };
        object[symbolKey] = { value: 2 };
        Object.defineProperty(object, "answer", {
            get: function getAnswer() { return this.data.value + 41; },
            set: function setAnswer(value) { this.data.value = value; },
            enumerable: true,
            configurable: true
        });

        var array = [{ dense: 1 }, , { dense: 3 }];
        array[4097] = { sparse: true };
        "#,
    )?;

    let snapshot = vm.object_edge_snapshot()?;
    ensure_positive(
        snapshot.count(VmObjectEdgeKind::Property),
        "object property edges",
    )?;
    ensure_positive(
        snapshot.count(VmObjectEdgeKind::Prototype),
        "object prototype edges",
    )?;
    ensure_snapshot_sum(snapshot)?;

    let value = vm.eval("object.answer")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn snapshots_boxed_proxy_and_typed_array_internal_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.eval(
        r#"
        var boxedString = new String("camera");
        var symbol = Symbol("slot");
        var boxedSymbol = Object(symbol);
        var target = { answer: 42 };
        var handler = {};
        var proxy = new Proxy(target, handler);
        var buffer = new ArrayBuffer(8);
        var bytes = new Uint8Array(buffer);
        bytes[0] = 42;
        "#,
    )?;

    let snapshot = vm.object_edge_snapshot()?;
    ensure_at_least(
        snapshot.count(VmObjectEdgeKind::InternalSlot),
        5,
        "object internal-slot edges",
    )?;
    ensure_snapshot_sum(snapshot)?;

    let value = vm.eval(
        "boxedString.length + (boxedSymbol.valueOf() === symbol ? 1 : 0) + proxy.answer + bytes[0]",
    )?;
    ensure_value(&value, &Value::Number(91.0))
}

#[test]
fn object_heap_caches_are_runtime_anchor_roots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let initial = vm.root_snapshot()?;
    ensure_usize(
        initial.count(VmRootKind::RuntimeAnchor),
        0,
        "initial runtime anchor roots",
    )?;

    vm.eval(
        r#"
        var key = Symbol("shape-key");
        var object = { alpha: 1 };
        object[key] = 2;
        var array = [3];
        "#,
    )?;
    let installed = vm.root_snapshot()?;
    ensure_positive(
        installed.count(VmRootKind::RuntimeAnchor),
        "object prototype and shape runtime anchors",
    )
}

fn ensure_snapshot_sum(snapshot: VmObjectEdgeSnapshot) -> TestResult {
    let summed = VmObjectEdgeKind::all()
        .iter()
        .try_fold(0_usize, |total, kind| {
            checked_sum(total, snapshot.count(*kind))
        })?;
    ensure_usize(summed, snapshot.total(), "object edge category sum")
}

fn checked_sum(total: usize, count: usize) -> Result<usize, Box<dyn std::error::Error>> {
    total
        .checked_add(count)
        .ok_or_else(|| "object edge snapshot sum overflowed".into())
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
