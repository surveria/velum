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

const EQUIVALENCE_CASES: &[(&str, &str)] = &[
    (
        "numeric-array-string",
        r#"
        let values = [1, 2, 3];
        values[1] = values[1] * 20;
        let dynamic = 2;
        values[dynamic] += 39;
        let text = "v:" + values[0] + ":" + values[1] + ":" + values[2];
        text + "|" + (-values[0]) + "|" + (values.length === 3)
        "#,
    ),
    (
        "bindings-closures",
        r#"
        let outer = 40;
        function make(delta) {
            let local = outer + delta;
            return function (step) {
                local = local + step;
                return local;
            };
        }
        let next = make(1);
        next(0) + ":" + next(1) + ":" + typeof missing + ":" + Number.isNaN(NaN)
        "#,
    ),
    (
        "calls-properties",
        r#"
        let record = { value: 40 };
        function add(left, right) {
            return left + right;
        }
        let first = add(record.value, 1);
        let second = add.call(undefined, first, 1);
        JSON.stringify({ result: second, keys: Object.keys(record).join(",") })
        "#,
    ),
    (
        "function-array-specializers",
        r#"
        let add = function namedAdd(left, right) {
            return left + right;
        };
        let mapped = [1, 2, 3].flatMap(function(value) {
            return [value + 1, value + 2];
        });
        let sorted = [4, 1, 3, 2].toSorted(function(left, right) {
            return left - right;
        });
        add(20, 22) + "|" + mapped.join(",") + "|" + sorted.join(",")
        "#,
    ),
    (
        "proxy-completion",
        r#"
        let writes = 0;
        let target = { value: 40 };
        let proxy = new Proxy(target, {
            get: function (object, key) {
                return object[key];
            },
            set: function (object, key, value) {
                writes = writes + 1;
                object[key] = value;
                return true;
            }
        });
        proxy.value = proxy.value + 2;
        let caught = "";
        try {
            missing();
        } catch (error) {
            caught = error.name;
        }
        proxy.value + ":" + writes + ":" + caught
        "#,
    ),
];

const ERROR_EQUIVALENCE_SOURCE: &str = r#"
function fail() {
    throw new TypeError("optimizer equivalence");
}
fail();
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

#[test]
fn optimizer_modes_match_across_semantic_clusters_and_output() -> TestResult {
    for (label, source) in EQUIVALENCE_CASES {
        let mut enabled = Vm::new();
        let mut disabled = disabled_vm();
        let script = enabled.compile(source)?;
        let enabled_result = enabled.eval_compiled_owned(&script)?;
        let disabled_result = disabled.eval_compiled_owned(&script)?;
        ensure(
            enabled_result == disabled_result,
            &format!(
                "optimizer modes diverged for {label}: enabled {enabled_result:?}, disabled {disabled_result:?}"
            ),
        )?;
        ensure(
            enabled.output() == disabled.output(),
            &format!("optimizer output diverged for {label}"),
        )?;
        ensure_disabled_snapshot(disabled.optimization_snapshot())?;
    }
    Ok(())
}

#[test]
fn optimizer_modes_preserve_uncaught_error_behavior() -> TestResult {
    let mut enabled = Vm::new();
    let mut disabled = disabled_vm();
    let script = enabled.compile(ERROR_EQUIVALENCE_SOURCE)?;
    let Err(enabled_error) = enabled.eval_compiled_owned(&script) else {
        return Err("enabled optimizer unexpectedly accepted the failing source".into());
    };
    let Err(disabled_error) = disabled.eval_compiled_owned(&script) else {
        return Err("disabled optimizer unexpectedly accepted the failing source".into());
    };
    ensure(
        enabled_error.to_string() == disabled_error.to_string(),
        &format!(
            "optimizer error behavior diverged: enabled {enabled_error}, disabled {disabled_error}"
        ),
    )?;
    ensure_disabled_snapshot(disabled.optimization_snapshot())
}

fn disabled_vm() -> Vm {
    Vm::with_config(VmConfig::default().with_optimization_mode(OptimizationMode::Disabled))
}

fn ensure_disabled_snapshot(snapshot: rs_quickjs::VmOptimizationSnapshot) -> TestResult {
    ensure(
        snapshot.mode() == OptimizationMode::Disabled,
        "wrong optimizer mode",
    )?;
    ensure_zero(
        snapshot.bytecode_linear_segment_runs(),
        "disabled linear segment runs",
    )?;
    ensure_zero(
        snapshot.bytecode_linear_direct_runs(),
        "disabled direct loop runs",
    )?;
    ensure_zero(
        snapshot.native_call_cache_hits(),
        "disabled native cache hits",
    )?;
    ensure_zero(
        snapshot.native_call_cache_misses(),
        "disabled native cache misses",
    )?;
    ensure_zero(
        snapshot.native_call_cache_slow_paths(),
        "disabled native cache slow paths",
    )?;
    ensure_zero(
        snapshot.call_value_cache_hits(),
        "disabled call-value cache hits",
    )?;
    ensure_zero(
        snapshot.call_value_cache_misses(),
        "disabled call-value cache misses",
    )?;
    ensure_zero(
        snapshot.call_value_cache_slow_paths(),
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
