use rs_quickjs::{Engine, EngineConfig, RuntimeLimits, Value, VmConfig};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bytecode_runs_direct_active_try_finally_benchmark_body() -> TestResult {
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
    let source = include_str!("corpora/benchmarks/active/try_finally.js");
    let script = vm.compile(source)?;
    let initial_direct_runs = vm.resource_usage().bytecode_linear_direct_runs;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(131_072.0))?;
    let direct_run_delta = vm
        .resource_usage()
        .bytecode_linear_direct_runs
        .checked_sub(initial_direct_runs)
        .ok_or("bytecode linear direct counter moved backwards")?;
    ensure_at_least(direct_run_delta, 65_536, "bytecode linear direct runs")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, min: usize, label: &str) -> TestResult {
    if actual >= min {
        return Ok(());
    }
    Err(format!("expected {label} >= {min}, got {actual}").into())
}
