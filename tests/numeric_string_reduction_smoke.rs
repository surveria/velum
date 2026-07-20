use velum::{Error, OptimizationMode, OwnedValue, RuntimeLimits, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const REDUCTION_SOURCE: &str = r#"
    var text = "A😀z";
    var total = 0;
    for (var index = 0; index < text.length; index = index + 1) {
        total = total + text.charCodeAt(index);
    }
    total
"#;

#[test]
fn numeric_string_reduction_matches_generic_utf16_execution() -> TestResult {
    let mut enabled = Vm::new();
    let mut disabled =
        Vm::with_config(VmConfig::default().with_optimization_mode(OptimizationMode::Disabled));
    let script = enabled.compile(REDUCTION_SOURCE)?;
    let enabled_value = enabled.eval_compiled_owned(&script)?;
    let disabled_value = disabled.eval_compiled_owned(&script)?;

    ensure_equal(&enabled_value, &OwnedValue::Number(112_376.0))?;
    ensure_equal(&enabled_value, &disabled_value)?;
    ensure_at_least(
        enabled
            .optimization_snapshot()
            .bytecode_linear_direct_runs(),
        12,
        "numeric string reduction direct runs",
    )
}

#[test]
fn numeric_string_reduction_declines_observable_fallbacks() -> TestResult {
    let cases = [
        r#"
            var calls = 0;
            var original = String.prototype.charCodeAt;
            Object.defineProperty(String.prototype, "charCodeAt", {
                configurable: true,
                get: function () {
                    calls = calls + 1;
                    return original;
                }
            });
            var text = "abc";
            var total = 0;
            for (var index = 0; index < text.length; index = index + 1) {
                total = total + text.charCodeAt(index);
            }
            total * 10 + calls
        "#,
        r#"
            String.prototype.charCodeAt = function (index) { return index + 1; };
            var text = "abc";
            var total = 0;
            for (var index = 0; index < text.length; index = index + 1) {
                total = total + text.charCodeAt(index);
            }
            total
        "#,
        r#"
            var text = new String("abc");
            var total = 0;
            for (var index = 0; index < text.length; index = index + 1) {
                total = total + text.charCodeAt(index);
            }
            total
        "#,
        r#"
            var text = "abc";
            var total = "";
            for (var index = 0; index < text.length; index = index + 1) {
                total = total + text.charCodeAt(index);
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
fn numeric_string_reduction_declines_immutable_bindings() -> TestResult {
    let source = r#"
        var text = "abc";
        const total = 0;
        for (var index = 0; index < text.length; index = index + 1) {
            total = total + text.charCodeAt(index);
        }
        total
    "#;
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
    Ok(())
}

#[test]
fn numeric_string_reduction_observes_method_changes_between_calls() -> TestResult {
    let source = r#"
        var text = "AZ";
        function scan() {
            var total = 0;
            for (var index = 0; index < text.length; index = index + 1) {
                total = total + text.charCodeAt(index);
            }
            return total;
        }
        var first = scan();
        String.prototype.charCodeAt = function (index) { return index + 1; };
        var second = scan();
        first * 100 + second
    "#;
    let enabled = eval_owned(source, OptimizationMode::Enabled)?;
    let disabled = eval_owned(source, OptimizationMode::Disabled)?;
    ensure_equal(&enabled, &OwnedValue::Number(15_503.0))?;
    ensure_equal(&enabled, &disabled)
}

#[test]
fn empty_numeric_string_reduction_does_not_read_body_property() -> TestResult {
    let source = r#"
        Object.defineProperty(String.prototype, "charCodeAt", {
            configurable: true,
            get: function () { throw new Error("must not run"); }
        });
        var text = "";
        var total = 7;
        for (var index = 0; index < text.length; index = index + 1) {
            total = total + text.charCodeAt(index);
        }
        total
    "#;
    ensure_equal(
        &eval_owned(source, OptimizationMode::Enabled)?,
        &OwnedValue::Number(7.0),
    )
}

#[test]
fn numeric_string_reduction_preserves_runtime_step_limits() -> TestResult {
    let source = r#"
        var text = "safe-rust-javascript";
        var total = 0;
        for (var index = 0; index < text.length; index = index + 1) {
            total = total + text.charCodeAt(index);
        }
        total
    "#;
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

fn ensure_at_least(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual >= minimum {
        return Ok(());
    }
    Err(format!("expected {label} >= {minimum}, got {actual}").into())
}
