use std::rc::Rc;

use parking_lot::Mutex;
use rs_quickjs::{Engine, Error, Value, VmRootKind, VmRootSnapshot};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn host_callbacks_observe_operand_and_call_roots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let captured = Rc::new(Mutex::new(None));
    let callback_capture = Rc::clone(&captured);
    vm.register_host_function_typed("captureTransientRoots", move |call| {
        *callback_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;

    let value = vm.eval(
        r#"
        var waiting = { value: 40 };
        function probe() {
            return waiting === captureTransientRoots({ value: 2 });
        }
        probe();
        "#,
    )?;
    ensure_value(&value, &Value::Bool(false))?;

    let snapshot = copied_snapshot(&captured, "call root snapshot")?;
    ensure_positive(
        snapshot.count(VmRootKind::TransientOperand),
        "transient operand roots",
    )?;
    ensure_positive(
        snapshot.count(VmRootKind::TransientCall),
        "transient call roots",
    )?;
    ensure_snapshot_sum(snapshot)?;
    ensure_transient_roots_cleared(vm.root_snapshot()?)
}

#[test]
fn protocol_callbacks_observe_iterator_temporary_roots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let captured = Rc::new(Mutex::new(None));
    let callback_capture = Rc::clone(&captured);
    vm.register_host_function_typed("captureIteratorRoots", move |call| {
        *callback_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;

    vm.eval(
        r#"
        var iterable = {};
        iterable[Symbol.iterator] = function iteratorFactory() {
            return {
                next: function next() {
                    captureIteratorRoots();
                    return { done: true };
                }
            };
        };
        for (var value of iterable) {
            value;
        }
        "#,
    )?;

    let snapshot = copied_snapshot(&captured, "iterator root snapshot")?;
    ensure_at_least(
        snapshot.count(VmRootKind::TransientTemporary),
        2,
        "iterator temporary roots",
    )?;
    ensure_positive(
        snapshot.count(VmRootKind::TransientOperand),
        "iterator operand roots",
    )?;
    ensure_positive(
        snapshot.count(VmRootKind::TransientCall),
        "iterator call roots",
    )?;
    ensure_snapshot_sum(snapshot)?;
    ensure_transient_roots_cleared(vm.root_snapshot()?)
}

#[test]
fn descriptor_getters_observe_prior_temporary_values() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let captured = Rc::new(Mutex::new(None));
    let callback_capture = Rc::clone(&captured);
    vm.register_host_function_typed("captureDescriptorRoots", move |call| {
        *callback_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;

    vm.eval(
        r#"
        var getter = function getter() { return 42; };
        var descriptor = {
            get: getter,
            get set() {
                captureDescriptorRoots();
                return undefined;
            },
            enumerable: true,
            configurable: true
        };
        var target = {};
        Object.defineProperty(target, "answer", descriptor);
        "#,
    )?;

    let snapshot = copied_snapshot(&captured, "descriptor root snapshot")?;
    ensure_positive(
        snapshot.count(VmRootKind::TransientTemporary),
        "descriptor temporary roots",
    )?;
    ensure_snapshot_sum(snapshot)?;
    ensure_transient_roots_cleared(vm.root_snapshot()?)
}

#[test]
fn proxy_result_processing_keeps_the_trap_result_rooted() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let captured = Rc::new(Mutex::new(None));
    let callback_capture = Rc::clone(&captured);
    vm.register_host_function_typed("captureProxyRoots", move |call| {
        *callback_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;

    let value = vm.eval(
        r#"
        var target = { answer: 42 };
        var proxy = new Proxy(target, {
            ownKeys: function ownKeys() {
                return new Proxy(["answer"], {
                    get: function get(result, property, receiver) {
                        captureProxyRoots();
                        return Reflect.get(result, property, receiver);
                    }
                });
            },
            getOwnPropertyDescriptor: function descriptor(object, property) {
                return Object.getOwnPropertyDescriptor(object, property);
            }
        });
        Object.keys(proxy)[0] === "answer";
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))?;

    let snapshot = copied_snapshot(&captured, "Proxy result root snapshot")?;
    ensure_positive(
        snapshot.count(VmRootKind::TransientTemporary),
        "Proxy result temporary roots",
    )?;
    ensure_snapshot_sum(snapshot)?;
    ensure_transient_roots_cleared(vm.root_snapshot()?)
}

#[test]
fn class_computed_keys_remain_operand_roots_during_coercion() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let captured = Rc::new(Mutex::new(None));
    let callback_capture = Rc::clone(&captured);
    vm.register_host_function_typed("captureClassRoots", move |call| {
        *callback_capture.lock() = Some(call.root_snapshot());
        Ok(0.0)
    })?;

    let value = vm.eval(
        r#"
        var computed = {
            toString: function toString() {
                captureClassRoots();
                return "answer";
            }
        };
        class Example {
            [computed]() { return 42; }
        }
        new Example().answer();
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))?;

    let snapshot = copied_snapshot(&captured, "class key root snapshot")?;
    ensure_positive(
        snapshot.count(VmRootKind::TransientOperand),
        "class computed-key operand roots",
    )?;
    ensure_snapshot_sum(snapshot)?;
    ensure_transient_roots_cleared(vm.root_snapshot()?)
}

#[test]
fn transient_scopes_clear_after_host_errors() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.register_host_function_typed("failWithTransientRoots", |_call| {
        Err::<f64, _>(Error::runtime("transient root probe failure"))
    })?;

    match vm.eval("({ held: true }) === failWithTransientRoots({ argument: true })") {
        Ok(value) => {
            return Err(format!("expected host failure, got {value:?}").into());
        }
        Err(_error) => {}
    }

    ensure_transient_roots_cleared(vm.root_snapshot()?)
}

fn ensure_transient_roots_cleared(snapshot: VmRootSnapshot) -> TestResult {
    ensure_usize(
        snapshot.count(VmRootKind::TransientOperand),
        0,
        "settled transient operand roots",
    )?;
    ensure_usize(
        snapshot.count(VmRootKind::TransientCall),
        0,
        "settled transient call roots",
    )?;
    ensure_usize(
        snapshot.count(VmRootKind::TransientTemporary),
        0,
        "settled transient temporary roots",
    )?;
    ensure_snapshot_sum(snapshot)
}

fn copied_snapshot(
    source: &Mutex<Option<VmRootSnapshot>>,
    label: &str,
) -> Result<VmRootSnapshot, Box<dyn std::error::Error>> {
    let snapshot = *source.lock();
    snapshot.ok_or_else(|| format!("expected {label} to be captured").into())
}

fn ensure_snapshot_sum(snapshot: VmRootSnapshot) -> TestResult {
    let summed = VmRootKind::all().iter().try_fold(0_usize, |total, kind| {
        checked_sum(total, snapshot.count(*kind))
    })?;
    ensure_usize(summed, snapshot.total(), "root category sum")
}

fn checked_sum(total: usize, count: usize) -> Result<usize, Box<dyn std::error::Error>> {
    total
        .checked_add(count)
        .ok_or_else(|| "root snapshot sum overflowed".into())
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
