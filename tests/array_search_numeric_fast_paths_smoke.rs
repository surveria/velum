use velum::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const ARRAY_SEARCH_NUMERIC_SCRIPT: &str = r"
let values = [NaN, -0, 1, 2, 1];
let packedOk =
    values.includes(NaN) &&
    values.indexOf(NaN) === -1 &&
    values.includes(+0) &&
    values.indexOf(+0) === 1 &&
    values.lastIndexOf(1) === 4 &&
    values.lastIndexOf(1, 3) === 2;

let sparse = Array(4);
sparse[3] = 7;
let holeyOk =
    sparse.includes(undefined) &&
    sparse.includes(7) &&
    sparse.indexOf(undefined) === -1 &&
    sparse.indexOf(7) === 3 &&
    sparse.lastIndexOf(undefined) === -1 &&
    sparse.lastIndexOf(7) === 3;

packedOk && holeyOk ? 42 : 0
";

#[test]
fn array_numeric_search_fast_paths_preserve_js_equality_rules() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(ARRAY_SEARCH_NUMERIC_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn compiled_array_search_fast_paths_reuse_validated_lengths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;
        for (var index = 0; index < 16; index++) {
            var values = [index, index + 1, index + 2, index + 1];
            total += values.includes(index + 1) ? 1 : 0;
            total += values.indexOf(index + 1, 2);
            total += values.lastIndexOf(index + 1);
            total += values.slice(1, 3).length;

            var sparse = Array(4);
            sparse[3] = index;
            total += sparse.includes(undefined) ? 1 : 0;
            total += sparse.indexOf(index);
            total += sparse.lastIndexOf(index);
        }
        total
        ",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(256.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(256.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected {expected}, got {actual}").into())
}
