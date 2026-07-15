use velum::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bytecode_runs_structured_update_expression_loop_body() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        let total = 0;
        let record = { value: 0 };
        let values = [1, 2, 3, 4];

        for (let index = 0; index < 128; index++) {
            total++;
            record.value++;
            ++values[index & 3];
            if ((index & 7) === 0) {
                --record.value;
            }
        }

        total + record.value + values[0] + values[1] + values[2] + values[3] === 378 ? 42 : 0
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
    ensure_at_least(direct_run_delta, 128, "bytecode linear direct runs")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }
    Err(format!("expected {label} >= {expected}, got {actual}").into())
}
