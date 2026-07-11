use rs_quickjs::{OptimizationMode, OwnedValue, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const EQUIVALENCE_SOURCE: &str = r#"
function add(left, right) {
    return left + right;
}
var values = [];
for (var index = 0; index < 256; index = index + 1) {
    values[index] = index * 3;
}
var total = 0;
for (var cursor = 0; cursor < values.length; cursor = cursor + 1) {
    total = total + values[cursor];
}
var record = { value: 7 };
for (var repeat = 0; repeat < 64; repeat = repeat + 1) {
    total = add(total, record.value);
}
var encoded = JSON.stringify({ total: total, length: values.length });
encoded + "|" + values[17] + "|" + (record.value in record)
"#;

#[test]
fn optimizer_mode_is_vm_local_and_generic_execution_is_equivalent() -> TestResult {
    let enabled_config = VmConfig::default();
    let disabled_config = VmConfig::default().with_optimization_mode(OptimizationMode::Disabled);
    let mut enabled = Vm::with_config(enabled_config.clone());
    let mut disabled = Vm::with_config(disabled_config.clone());
    let script = enabled.compile(EQUIVALENCE_SOURCE)?;

    let enabled_result = enabled.eval_compiled_owned(&script)?;
    let disabled_result = disabled.eval_compiled_owned(&script)?;
    ensure(
        enabled_result == disabled_result,
        &format!(
            "optimizer modes diverged: enabled {enabled_result:?}, disabled {disabled_result:?}"
        ),
    )?;
    ensure(
        matches!(enabled_result, OwnedValue::String(_)),
        "equivalence workload did not produce an owned string",
    )?;
    ensure(
        enabled.config() == enabled_config,
        "enabled VM config changed after evaluation",
    )?;
    ensure(
        disabled.config() == disabled_config,
        "disabled VM config changed after evaluation",
    )?;

    let enabled_snapshot = enabled.optimization_snapshot();
    ensure(
        enabled_snapshot.mode() == OptimizationMode::Enabled,
        "enabled VM reported the wrong optimizer mode",
    )?;
    ensure(
        enabled_snapshot.bytecode_linear_segment_runs() > 0
            || enabled_snapshot.bytecode_linear_direct_runs() > 0,
        "enabled VM did not exercise an optimized bytecode path",
    )?;

    let disabled_snapshot = disabled.optimization_snapshot();
    ensure(
        disabled_snapshot.mode() == OptimizationMode::Disabled,
        "disabled VM reported the wrong optimizer mode",
    )?;
    ensure_zero(
        disabled_snapshot.bytecode_linear_segment_runs(),
        "disabled linear segment runs",
    )?;
    ensure_zero(
        disabled_snapshot.bytecode_linear_direct_runs(),
        "disabled direct loop runs",
    )?;
    ensure_zero(
        disabled_snapshot.native_call_cache_hits(),
        "disabled native cache hits",
    )?;
    ensure_zero(
        disabled_snapshot.native_call_cache_misses(),
        "disabled native cache misses",
    )?;
    ensure_zero(
        disabled_snapshot.native_call_cache_slow_paths(),
        "disabled native cache slow paths",
    )?;
    ensure_zero(
        disabled_snapshot.call_value_cache_hits(),
        "disabled call-value cache hits",
    )?;
    ensure_zero(
        disabled_snapshot.call_value_cache_misses(),
        "disabled call-value cache misses",
    )?;
    ensure_zero(
        disabled_snapshot.call_value_cache_slow_paths(),
        "disabled call-value cache slow paths",
    )
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.to_owned().into())
}

fn ensure_zero(value: usize, label: &str) -> TestResult {
    ensure(
        value == 0,
        &format!("expected {label} to be zero, got {value}"),
    )
}
