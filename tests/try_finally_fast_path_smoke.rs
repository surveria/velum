use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn structured_loop_preserves_try_finally_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let source = r"
        let total = 0;
        for (let round = 0; round < 4; round++) {
            for (let index = 0; index < 4; index++) {
                try {
                    if ((index & 3) === 0) { throw 1; }
                    total += 1;
                } catch (error) {
                    total += error;
                } finally {
                    total += 1;
                }
            }
        }
        total
    ";
    let script = vm.compile(source)?;
    let initial_direct_runs = vm.resource_usage().bytecode_linear_direct_runs;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(32.0))?;
    let direct_run_delta = vm
        .resource_usage()
        .bytecode_linear_direct_runs
        .checked_sub(initial_direct_runs)
        .ok_or("bytecode linear direct counter moved backwards")?;
    ensure_at_least(direct_run_delta, 16, "bytecode linear direct runs")
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
