use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn structured_while_preserves_array_sum_semantics() -> TestResult {
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
    ensure_at_least(direct_run_delta, 256, "bytecode linear direct runs")
}

#[test]
fn nested_structured_while_preserves_binding_updates() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let source = r"
        let rounds = 0;
        let grandTotal = 0;
        while (rounds < 4) {
            let values = [1, 2, 3, 4];
            let index = 0;
            let total = 0;
            while (index < 100) {
                var slot = index & 3;
                total += values[slot];
                index += 1;
            }
            grandTotal += total;
            rounds += 1;
        }
        grandTotal
    ";
    let script = vm.compile(source)?;
    let initial_direct_runs = vm.resource_usage().bytecode_linear_direct_runs;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(1_000.0))?;
    let direct_run_delta = vm
        .resource_usage()
        .bytecode_linear_direct_runs
        .checked_sub(initial_direct_runs)
        .ok_or("bytecode linear direct counter moved backwards")?;
    ensure_at_least(direct_run_delta, 400, "bytecode linear direct runs")
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
