use rs_quickjs::{Engine, EngineConfig, RuntimeLimits, Value, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn batches_constructor_prototype_benchmark_loop() -> TestResult {
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
    let source = include_str!("corpora/benchmarks/active/constructor_prototypes.js");
    let script = vm.compile(source)?;

    let before = vm.resource_usage();
    let value = vm.eval_compiled(&script)?;
    let after = vm.resource_usage();

    ensure_value(&value, &Value::Number(1_440_002_400_000.0))?;
    let direct_runs = after
        .bytecode_linear_direct_runs
        .checked_sub(before.bytecode_linear_direct_runs)
        .ok_or("bytecode direct run counter moved backwards")?;
    if direct_runs < 1_200_000 {
        return Err(format!(
            "expected at least 1200000 direct bytecode loop runs, got {direct_runs}"
        )
        .into());
    }
    Ok(())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
