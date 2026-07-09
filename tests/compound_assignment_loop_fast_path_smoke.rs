use rs_quickjs::{Engine, EngineConfig, RuntimeLimits, Value, VmConfig};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bytecode_batches_active_compound_assignment_benchmark_loop() -> TestResult {
    const BENCH_RUNTIME_LIMITS: RuntimeLimits = RuntimeLimits {
        max_source_len: 262_144,
        max_statements: 65_536,
        max_expression_depth: 512,
        max_runtime_steps: 100_000_000,
        max_string_len: 1_048_576,
        max_bindings: 65_536,
        max_objects: 1_000_000,
        max_object_properties: 1_000_000,
    };

    let engine = Engine::with_config(EngineConfig::with_default_vm_config(VmConfig::with_limits(
        BENCH_RUNTIME_LIMITS,
    )));
    let mut vm = engine.create_vm();
    let script = vm.compile(include_str!(
        "corpora/benchmarks/active/compound_assignment.js"
    ))?;
    let before = vm.resource_usage();
    let value = vm.eval_compiled(&script)?;
    let after = vm.resource_usage();

    ensure_value(&value, &Value::Number(253_963.0))?;
    let segment_delta = after
        .bytecode_linear_segment_runs
        .checked_sub(before.bytecode_linear_segment_runs)
        .ok_or("segment counter moved backwards")?;
    let direct_delta = after
        .bytecode_linear_direct_runs
        .checked_sub(before.bytecode_linear_direct_runs)
        .ok_or("direct counter moved backwards")?;
    ensure_usize(segment_delta, 0, "bytecode linear segment runs")?;
    ensure_at_least(direct_delta, 65_536, "bytecode linear direct runs")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected}, got {actual}").into())
}

fn ensure_at_least(actual: usize, min: usize, label: &str) -> TestResult {
    if actual >= min {
        return Ok(());
    }
    Err(format!("expected {label} >= {min}, got {actual}").into())
}
