use rs_quickjs::{Engine, VmGcKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn marks_strong_roots_and_reports_unreachable_records() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let retained = vm.eval_retained("({ retained: { value: 42 } })")?;
    vm.eval(
        r#"
        var rooted = { child: { value: 7 } };
        function capture() {
            return rooted.child;
        }
        (function createGarbage() {
            let discarded = { child: { value: 1 } };
            return discarded.value;
        })();
        42
        "#,
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
    )
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
    )
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
