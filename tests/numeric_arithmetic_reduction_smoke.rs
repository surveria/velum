use velum::{Error, OptimizationMode, OwnedValue, RuntimeLimits, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const REDUCTION_SOURCE: &str = r"
    var total = 0;
    for (var index = 0; index < 64; index = index + 1) {
        total = total + (((index * 3) + 7) & 255);
    }
    total
";

#[test]
fn numeric_arithmetic_reduction_matches_generic_execution_and_steps() -> TestResult {
    let mut enabled = Vm::new();
    let mut disabled =
        Vm::with_config(VmConfig::default().with_optimization_mode(OptimizationMode::Disabled));
    let script = enabled.compile(REDUCTION_SOURCE)?;
    let enabled_value = enabled.eval_compiled_owned(&script)?;
    let disabled_value = disabled.eval_compiled_owned(&script)?;

    ensure_equal(&enabled_value, &OwnedValue::Number(6_496.0))?;
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
        "numeric arithmetic reduction direct runs",
    )
}

#[test]
fn numeric_arithmetic_reduction_preserves_number_edge_cases() -> TestResult {
    let cases = [
        r"
            var total = 0 / 0;
            for (var index = 0; index < 8; index = index + 1) {
                total = total + ((index * 3) & 7);
            }
            Number.isNaN(total) ? 42 : 0
        ",
        r"
            var total = 1;
            for (var index = 0; index < 8; index = index + 1) {
                total = ((total * 5) + index) >>> 0;
            }
            total
        ",
        r"
            var total = 3;
            for (var index = 0; index < 6; index = index + 1) {
                total = ((total ** 2) % 257) - index;
            }
            total
        ",
    ];
    for source in cases {
        ensure_modes_match(source)?;
    }
    Ok(())
}

#[test]
fn numeric_arithmetic_reduction_declines_observable_inputs() -> TestResult {
    let cases = [
        r"
            var conversions = 0;
            var factor = {
                valueOf: function () {
                    conversions = conversions + 1;
                    return 2;
                }
            };
            var total = 0;
            for (var index = 0; index < 4; index = index + 1) {
                total = total + (index * factor);
            }
            total * 100 + conversions
        ",
        r"
            var conversions = 0;
            var total = {
                valueOf: function () {
                    conversions = conversions + 1;
                    return 0;
                }
            };
            for (var index = 0; index < 4; index = index + 1) {
                total = total + index;
            }
            total * 100 + conversions
        ",
        r"
            var total = 0;
            var scope = { factor: 3 };
            with (scope) {
                for (var index = 0; index < 4; index = index + 1) {
                    total = total + (index * factor);
                }
            }
            total
        ",
    ];
    for source in cases {
        ensure_modes_match(source)?;
    }
    Ok(())
}

#[test]
fn empty_numeric_arithmetic_reduction_does_not_assign_immutable_state() -> TestResult {
    let source = r"
        const total = 7;
        for (const index = 0; index < 0; index = index + 1) {
            total = total + index;
        }
        total
    ";
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        ensure_equal(&eval_owned(source, mode)?, &OwnedValue::Number(7.0))?;
    }
    Ok(())
}

#[test]
fn numeric_arithmetic_reduction_declines_active_immutable_bindings() -> TestResult {
    let sources = [
        r"
            const total = 0;
            for (var index = 0; index < 2; index = index + 1) {
                total = total + index;
            }
            total
        ",
        r"
            var total = 0;
            for (const index = 0; index < 2; index = index + 1) {
                total = total + index;
            }
            total
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
fn numeric_arithmetic_reduction_preserves_runtime_step_limits() -> TestResult {
    let limits = RuntimeLimits {
        max_runtime_steps: 100,
        ..RuntimeLimits::default()
    };
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let config = VmConfig::with_limits(limits.clone()).with_optimization_mode(mode);
        let mut vm = Vm::with_config(config);
        let script = vm.compile(REDUCTION_SOURCE)?;
        let error = vm
            .eval_compiled_owned(&script)
            .err()
            .ok_or("expected runtime step limit failure")?;
        if !matches!(error, Error::ResourceLimit { .. }) {
            return Err(format!("expected resource limit in {mode:?} mode, got {error}").into());
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
