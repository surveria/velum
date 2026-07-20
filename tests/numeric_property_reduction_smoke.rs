use velum::{Error, OptimizationMode, OwnedValue, RuntimeLimits, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const REDUCTION_SOURCE: &str = r"
    var record = { alpha: 3, beta: 5, gamma: 7 };
    var total = 0;
    for (var index = 0; index < 64; index = index + 1) {
        total = total + record.alpha;
        total = total + record.beta;
        total = total + record.gamma;
    }
    total
";

#[test]
fn numeric_property_reduction_matches_generic_execution_and_steps() -> TestResult {
    let mut enabled = Vm::new();
    let mut disabled =
        Vm::with_config(VmConfig::default().with_optimization_mode(OptimizationMode::Disabled));
    let script = enabled.compile(REDUCTION_SOURCE)?;
    let enabled_value = enabled.eval_compiled_owned(&script)?;
    let disabled_value = disabled.eval_compiled_owned(&script)?;

    ensure_equal(&enabled_value, &OwnedValue::Number(960.0))?;
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
        "numeric property reduction direct runs",
    )
}

#[test]
fn numeric_property_reduction_declines_observable_fallbacks() -> TestResult {
    let cases = [
        r#"
            var reads = 0;
            var observed = "";
            var record = {
                get first() { reads = reads + 1; return 1; },
                get second() {
                    reads = reads + 1;
                    observed = observed + total + ",";
                    return 2;
                }
            };
            var total = 0;
            for (var index = 0; index < 2; index = index + 1) {
                total = total + record.first;
                total = total + record.second;
            }
            total + ":" + reads + ":" + observed
        "#,
        r#"
            var reads = 0;
            var record = new Proxy({ first: 1, second: 2 }, {
                get: function (target, property) {
                    reads = reads + 1;
                    return target[property];
                }
            });
            var total = 0;
            for (var index = 0; index < 2; index = index + 1) {
                total = total + record.first;
                total = total + record.second;
            }
            total + ":" + reads
        "#,
        r#"
            var record = { first: 1, second: "x" };
            var total = 0;
            for (var index = 0; index < 2; index = index + 1) {
                total = total + record.first;
                total = total + record.second;
            }
            total
        "#,
    ];
    for source in cases {
        ensure_modes_match(source)?;
    }
    Ok(())
}

#[test]
fn numeric_property_reduction_observes_shape_and_prototype_changes() -> TestResult {
    let source = r"
        var record = { first: 1, second: 2 };
        function scan() {
            var total = 0;
            for (var index = 0; index < 4; index = index + 1) {
                total = total + record.first;
                total = total + record.second;
            }
            return total;
        }
        var first = scan();
        record.first = 10;
        delete record.second;
        record.__proto__ = { second: 5 };
        var second = scan();
        first * 100 + second
    ";
    let enabled = eval_owned(source, OptimizationMode::Enabled)?;
    let disabled = eval_owned(source, OptimizationMode::Disabled)?;
    ensure_equal(&enabled, &OwnedValue::Number(1_260.0))?;
    ensure_equal(&enabled, &disabled)
}

#[test]
fn empty_numeric_property_reduction_does_not_read_body_properties() -> TestResult {
    let source = r#"
        var record = {};
        Object.defineProperty(record, "first", {
            get: function () { throw new Error("must not run"); }
        });
        Object.defineProperty(record, "second", {
            get: function () { throw new Error("must not run"); }
        });
        var total = 7;
        for (var index = 0; index < 0; index = index + 1) {
            total = total + record.first;
            total = total + record.second;
        }
        total
    "#;
    ensure_equal(
        &eval_owned(source, OptimizationMode::Enabled)?,
        &OwnedValue::Number(7.0),
    )
}

#[test]
fn numeric_property_reduction_declines_immutable_bindings() -> TestResult {
    let sources = [
        r"
            var record = { first: 1, second: 2 };
            const total = 0;
            for (var index = 0; index < 2; index = index + 1) {
                total = total + record.first;
                total = total + record.second;
            }
            total
        ",
        r"
            var record = { first: 1, second: 2 };
            var total = 0;
            for (const index = 0; index < 2; index = index + 1) {
                total = total + record.first;
                total = total + record.second;
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
fn numeric_property_reduction_preserves_runtime_step_limits() -> TestResult {
    let source = r"
        var record = { first: 1, second: 2 };
        var total = 0;
        for (var index = 0; index < 20; index = index + 1) {
            total = total + record.first;
            total = total + record.second;
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
