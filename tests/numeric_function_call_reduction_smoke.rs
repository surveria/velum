use velum::{Error, OptimizationMode, OwnedValue, RuntimeLimits, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const REDUCTION_SOURCE: &str = r"
    function step(value) {
        return ((value * 5) + 11) & 65535;
    }
    var value = 1;
    for (var index = 0; index < 64; index = index + 1) {
        value = step(value);
    }
    value
";

#[test]
fn numeric_function_call_reduction_matches_generic_execution_and_steps() -> TestResult {
    let mut enabled = Vm::new();
    let mut disabled =
        Vm::with_config(VmConfig::default().with_optimization_mode(OptimizationMode::Disabled));
    let script = enabled.compile(REDUCTION_SOURCE)?;
    let enabled_value = enabled.eval_compiled_owned(&script)?;
    let disabled_value = disabled.eval_compiled_owned(&script)?;

    ensure_equal(&enabled_value, &OwnedValue::Number(62_529.0))?;
    ensure_equal(&enabled_value, &disabled_value)?;
    ensure_equal_usize(
        enabled.resource_usage().runtime_steps,
        disabled.resource_usage().runtime_steps,
        "runtime steps",
    )?;
    ensure_at_least(
        enabled
            .optimization_snapshot()
            .bytecode_linear_direct_runs(),
        192,
        "numeric function call reduction direct runs",
    )
}

#[test]
fn numeric_function_call_reduction_declines_observable_callees() -> TestResult {
    let cases = [
        r"
            var calls = 0;
            function step(value) {
                calls = calls + 1;
                return value + 1;
            }
            var value = 0;
            for (var index = 0; index < 4; index = index + 1) {
                value = step(value);
            }
            value * 100 + calls
        ",
        r"
            var calls = 0;
            var target = function (value) { return value + 1; };
            var step = new Proxy(target, {
                apply: function (callee, receiver, args) {
                    calls = calls + 1;
                    return Reflect.apply(callee, receiver, args);
                }
            });
            var value = 0;
            for (var index = 0; index < 4; index = index + 1) {
                value = step(value);
            }
            value * 100 + calls
        ",
        r"
            function step(value, other) { return value + other; }
            var value = 0;
            for (var index = 0; index < 4; index = index + 1) {
                value = step(value);
            }
            Number.isNaN(value) ? 42 : 0
        ",
    ];
    for source in cases {
        ensure_modes_match(source)?;
    }
    Ok(())
}

#[test]
fn numeric_function_call_reduction_observes_reassigned_functions() -> TestResult {
    let source = r"
        var step = function (value) { return value + 1; };
        function scan(seed) {
            var value = seed;
            for (var index = 0; index < 4; index = index + 1) {
                value = step(value);
            }
            return value;
        }
        var first = scan(0);
        step = function (value) { return value * 2; };
        var second = scan(1);
        first * 100 + second
    ";
    let enabled = eval_owned(source, OptimizationMode::Enabled)?;
    let disabled = eval_owned(source, OptimizationMode::Disabled)?;
    ensure_equal(&enabled, &OwnedValue::Number(416.0))?;
    ensure_equal(&enabled, &disabled)
}

#[test]
fn empty_numeric_function_call_reduction_does_not_resolve_callee() -> TestResult {
    let source = r#"
        var reads = 0;
        Object.defineProperty(globalThis, "step", {
            configurable: true,
            get: function () {
                reads = reads + 1;
                throw new Error("must not run");
            }
        });
        var value = 7;
        for (var index = 0; index < 0; index = index + 1) {
            value = step(value);
        }
        reads * 100 + value
    "#;
    ensure_equal(
        &eval_owned(source, OptimizationMode::Enabled)?,
        &OwnedValue::Number(7.0),
    )
}

#[test]
fn numeric_function_call_reduction_declines_immutable_bindings() -> TestResult {
    let sources = [
        r"
            function step(value) { return value + 1; }
            const value = 0;
            for (var index = 0; index < 2; index = index + 1) {
                value = step(value);
            }
            value
        ",
        r"
            function step(value) { return value + 1; }
            var value = 0;
            for (const index = 0; index < 2; index = index + 1) {
                value = step(value);
            }
            value
        ",
    ];
    for source in sources {
        for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
            let error = eval_owned(source, mode)
                .err()
                .ok_or("expected immutable assignment failure")?;
            if !error.to_string().contains("assignment to constant") {
                return Err(
                    format!("expected immutable assignment in {mode:?} mode, got {error}").into(),
                );
            }
        }
    }
    Ok(())
}

#[test]
fn numeric_function_call_reduction_preserves_runtime_and_stack_limits() -> TestResult {
    let runtime_limits = RuntimeLimits {
        max_runtime_steps: 100,
        ..RuntimeLimits::default()
    };
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let error = eval_with_limits(REDUCTION_SOURCE, mode, runtime_limits.clone())
            .err()
            .ok_or("expected runtime step limit failure")?;
        if !matches!(error, Error::ResourceLimit { .. }) {
            return Err(format!("expected resource limit in {mode:?} mode, got {error}").into());
        }
    }

    let stack_limits = RuntimeLimits {
        max_call_depth: 0,
        ..RuntimeLimits::default()
    };
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let error = eval_with_limits(REDUCTION_SOURCE, mode, stack_limits.clone())
            .err()
            .ok_or("expected call stack limit failure")?;
        if !error
            .to_string()
            .contains("Maximum call stack size exceeded")
        {
            return Err(format!("expected call stack limit in {mode:?} mode, got {error}").into());
        }
    }
    Ok(())
}

fn ensure_modes_match(source: &str) -> TestResult {
    let enabled = eval_owned(source, OptimizationMode::Enabled)?;
    let disabled = eval_owned(source, OptimizationMode::Disabled)?;
    ensure_equal(&enabled, &disabled)
}

fn eval_owned(source: &str, mode: OptimizationMode) -> Result<OwnedValue, Error> {
    let mut vm = Vm::with_config(VmConfig::default().with_optimization_mode(mode));
    vm.eval_owned(source)
}

fn eval_with_limits(
    source: &str,
    mode: OptimizationMode,
    limits: RuntimeLimits,
) -> Result<OwnedValue, Error> {
    let config = VmConfig::with_limits(limits).with_optimization_mode(mode);
    let mut vm = Vm::with_config(config);
    vm.eval_owned(source)
}

fn ensure_equal(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_equal_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected}, got {actual}").into())
}

fn ensure_at_least(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual >= minimum {
        return Ok(());
    }
    Err(format!("expected {label} >= {minimum}, got {actual}").into())
}
