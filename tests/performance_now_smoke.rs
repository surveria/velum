use std::{cell::Cell, rc::Rc, time::Duration};

use velum::{Engine, Runtime, Value, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn clock_reader(state: &Rc<Cell<Duration>>) -> impl Fn() -> Duration + 'static {
    let state = Rc::clone(state);
    move || state.get()
}

fn ensure_number(value: &Value, expected: f64) -> TestResult {
    let Value::Number(actual) = value else {
        return Err(format!("expected numeric clock value, got {value:?}").into());
    };
    if (*actual - expected).abs() <= f64::EPSILON {
        return Ok(());
    }
    Err(format!("expected clock value {expected}, got {actual}").into())
}

#[test]
fn exposes_vm_local_monotonic_performance_now() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const first = performance.now();
        const second = performance.now();
        const methodDescriptor = Object.getOwnPropertyDescriptor(performance, "now");
        const globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "performance");

        typeof performance === "object" &&
            performance === globalThis.performance &&
            Object.getPrototypeOf(performance) === Object.prototype &&
            typeof performance.now === "function" &&
            performance.now.name === "now" &&
            performance.now.length === 0 &&
            Number.isFinite(first) &&
            Number.isFinite(second) &&
            first >= 0 &&
            second >= first &&
            methodDescriptor.writable === true &&
            methodDescriptor.enumerable === false &&
            methodDescriptor.configurable === true &&
            globalDescriptor.value === performance &&
            globalDescriptor.writable === true &&
            globalDescriptor.enumerable === false &&
            globalDescriptor.configurable === true
            ? 42
            : 0
        "#,
    )?;

    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("performance surface check returned {value:?}").into())
}

#[test]
fn injected_clock_is_exact_and_clamps_regressions() -> TestResult {
    let base = Duration::from_secs(5);
    let state = Rc::new(Cell::new(base));
    let mut vm = Vm::with_config_and_clock(VmConfig::default(), clock_reader(&state));

    state.set(base + Duration::from_micros(1_250));
    ensure_number(&vm.eval("performance.now()")?, 1.25)?;

    state.set(base + Duration::from_micros(3_500));
    ensure_number(&vm.eval("performance.now()")?, 3.5)?;

    state.set(base + Duration::from_millis(2));
    ensure_number(&vm.eval("performance.now()")?, 3.5)
}

#[test]
fn independent_vms_capture_independent_clock_origins() -> TestResult {
    let state = Rc::new(Cell::new(Duration::from_secs(10)));
    let engine = Engine::new();
    let mut first = engine.create_vm_with_clock(clock_reader(&state));

    state.set(Duration::from_secs(12));
    let mut second = engine.create_vm_with_clock(clock_reader(&state));

    state.set(Duration::from_millis(12_500));
    ensure_number(&first.eval("performance.now()")?, 2_500.0)?;
    ensure_number(&second.eval("performance.now()")?, 500.0)
}

#[test]
fn runtime_context_accepts_a_deterministic_clock() -> TestResult {
    let state = Rc::new(Cell::new(Duration::from_secs(20)));
    let runtime = Runtime::new();
    let mut context = runtime.context_with_clock(clock_reader(&state));

    state.set(Duration::from_millis(20_125));
    ensure_number(&context.eval("performance.now()")?, 125.0)
}
