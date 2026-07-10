use rs_quickjs::{Engine, EngineConfig, RuntimeLimits, Value, VmConfig};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bytecode_runs_direct_simple_while_array_sum_body() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        let values = [1, 2, 3, 4];
        let index = 0;
        let total = 0;

        while (index < 256) {
          var slot = index & 3;
          total = total + values[slot];
          index = index + 1;
        }

        total === 640 && index === 256 && slot === 3 ? 42 : 0
        ",
    )?;
    let initial_direct_runs = vm.resource_usage().bytecode_linear_direct_runs;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let direct_run_delta = vm
        .resource_usage()
        .bytecode_linear_direct_runs
        .checked_sub(initial_direct_runs)
        .ok_or("bytecode linear direct counter moved backwards")?;
    ensure_between(direct_run_delta, 256, 300, "bytecode linear direct runs")
}

#[test]
fn bytecode_runs_direct_active_while_benchmark_body() -> TestResult {
    const BENCH_RUNTIME_LIMITS: RuntimeLimits = RuntimeLimits {
        max_source_len: 262_144,
        max_statements: 65_536,
        max_expression_depth: 512,
        max_runtime_steps: 100_000_000,
        max_string_len: 1_048_576,
        max_bindings: 65_536,
        max_objects: 1_000_000,
        max_object_properties: 1_000_000,
        storage: rs_quickjs::VmStorageLimits::unlimited(),
    };

    let engine = Engine::with_config(EngineConfig::with_default_vm_config(VmConfig::with_limits(
        BENCH_RUNTIME_LIMITS,
    )));
    let mut vm = engine.create_vm();
    let source = include_str!("corpora/benchmarks/active/while_statements.js");
    let script = vm.compile(source)?;
    let initial_direct_runs = vm.resource_usage().bytecode_linear_direct_runs;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(248_750_000.0))?;
    let direct_run_delta = vm
        .resource_usage()
        .bytecode_linear_direct_runs
        .checked_sub(initial_direct_runs)
        .ok_or("bytecode linear direct counter moved backwards")?;
    ensure_between(
        direct_run_delta,
        99_500_000,
        99_530_000,
        "bytecode linear direct runs",
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_between(actual: usize, min: usize, max: usize, label: &str) -> TestResult {
    if actual >= min && actual <= max {
        return Ok(());
    }
    Err(format!("expected {label} between {min} and {max}, got {actual}").into())
}
