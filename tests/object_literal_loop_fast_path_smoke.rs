use velum::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bytecode_runs_structured_object_literal_loop_body() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        let total = 0;

        for (let index = 0; index < 128; index = index + 1) {
            let object = {
                first: index,
                second: index + 1,
                nested: { value: index + 2 },
            };
            object.first = object.first + object.second;
            object.second = object.first + object.nested.value;
            object.nested.value = object.second + object.first;
            total = total + object.first + object.second + object.nested.value;
        }

        total === 82304 ? 42 : 0
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
