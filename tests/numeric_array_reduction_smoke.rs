use rs_quickjs::{Error, OptimizationMode, OwnedValue, RuntimeLimits, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const REDUCTION_SOURCE: &str = r"
    var samples = [3, 5, 7, 11, 13, 3];
    var sum = 0;
    for (var cursor = 0; cursor < samples.length; cursor++) {
        sum = sum + samples[cursor];
    }
    sum
";

#[test]
fn numeric_array_reduction_matches_generic_execution() -> TestResult {
    let mut enabled = Vm::new();
    let mut disabled =
        Vm::with_config(VmConfig::default().with_optimization_mode(OptimizationMode::Disabled));
    let script = enabled.compile(REDUCTION_SOURCE)?;
    let usage = script.usage();
    ensure_at_least(
        usage.bytecode_linear_peephole_candidate_count(),
        2,
        "compiled linear peephole candidates",
    )?;
    ensure_at_least(
        usage.bytecode_numeric_array_reduction_role_count(),
        3,
        "compiled numeric-array reduction roles",
    )?;
    let enabled_value = enabled.eval_compiled_owned(&script)?;
    let disabled_value = disabled.eval_compiled_owned(&script)?;

    ensure_equal(&enabled_value, &OwnedValue::Number(42.0))?;
    ensure_equal(&enabled_value, &disabled_value)?;
    let snapshot = enabled.optimization_snapshot();
    if snapshot.bytecode_linear_direct_runs() < 18 {
        return Err(format!(
            "expected reusable reduction plan counters, got {} direct runs",
            snapshot.bytecode_linear_direct_runs()
        )
        .into());
    }
    Ok(())
}

#[test]
fn compiled_linear_templates_are_reused_across_vms() -> TestResult {
    let compiler = Vm::new();
    let script = compiler.compile(REDUCTION_SOURCE)?;
    let usage = script.usage();
    ensure_at_least(
        usage.bytecode_linear_peephole_candidate_count(),
        2,
        "reusable linear peephole candidates",
    )?;
    ensure_at_least(
        usage.bytecode_numeric_array_reduction_role_count(),
        3,
        "reusable numeric-array reduction roles",
    )?;

    for _ in 0..4 {
        let mut vm = Vm::new();
        for _ in 0..3 {
            let value = vm.eval_compiled_owned(&script)?;
            ensure_equal(&value, &OwnedValue::Number(42.0))?;
        }
        ensure_at_least(
            vm.optimization_snapshot().bytecode_linear_direct_runs(),
            54,
            "cross-VM reusable reduction runs",
        )?;
    }
    Ok(())
}

#[test]
fn numeric_array_reduction_declines_observable_array_fallbacks() -> TestResult {
    let cases = [
        r"
        Array.prototype[1] = 40;
        var values = [1, , 1];
        var total = 0;
        for (var index = 0; index < values.length; index++) {
            total = total + values[index];
        }
        delete Array.prototype[1];
        total
        ",
        r#"
        var values = ["a", "b"];
        var total = "";
        for (var index = 0; index < values.length; index = index + 1) {
            total = total + values[index];
        }
        total
        "#,
    ];
    for source in cases {
        let mut enabled = Vm::new();
        let mut disabled =
            Vm::with_config(VmConfig::default().with_optimization_mode(OptimizationMode::Disabled));
        let script = enabled.compile(source)?;
        let enabled_value = enabled.eval_compiled_owned(&script)?;
        let disabled_value = disabled.eval_compiled_owned(&script)?;
        ensure_equal(&enabled_value, &disabled_value)?;
    }
    Ok(())
}

#[test]
fn numeric_array_reduction_preserves_runtime_step_limits() -> TestResult {
    let source = r"
        var values = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
                      11, 12, 13, 14, 15, 16, 17, 18, 19, 20];
        var total = 0;
        for (var index = 0; index < values.length; index++) {
            total = total + values[index];
        }
        total
    ";
    let limits = RuntimeLimits {
        max_runtime_steps: 100,
        ..RuntimeLimits::default()
    };
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let config = VmConfig::with_limits(limits.clone()).with_optimization_mode(mode);
        let mut vm = Vm::with_config(config);
        let script = vm.compile(source)?;
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

fn ensure_equal(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual >= minimum {
        return Ok(());
    }
    Err(format!("expected {label} >= {minimum}, got {actual}").into())
}
