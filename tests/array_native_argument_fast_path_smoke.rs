use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const ARRAY_NATIVE_ARGUMENT_FAST_PATH_SCRIPT: &str = r"
let order = '';
let mark = function(label, value) {
    order = order + label;
    return value;
};

let run = function() {
    let joinOk = [1, 2].join(mark('a', '-'), mark('b', 'unused')) === '1-2';
    let includesOk = [1, 2].includes(mark('c', 2), mark('d', 0), mark('e', 99)) === true;
    let indexOfOk = [1, 2].indexOf(mark('f', 2), mark('g', 0), mark('h', 99)) === 1;
    let lastIndexOfOk = [1, 2, 2].lastIndexOf(mark('i', 2), mark('j', 1), mark('k', 99)) === 1;
    let sliceOk = [1, 2, 3].slice(mark('l', 1), mark('m', 3), mark('n', 99)).join('|') === '2|3';

    let reversed = [1, 2];
    let reverseOk = reversed.reverse(mark('o', 0), mark('p', 0)).join('|') === '2|1';
    let popOk = [7].pop(mark('q', 0), mark('r', 0)) === 7;
    let shiftOk = [8].shift(mark('s', 0), mark('t', 0)) === 8;
    let pushed = [1];
    let pushOk = pushed.push(mark('u', 2), mark('v', 3)) === 3 &&
        pushed.join('|') === '1|2|3';
    let unshifted = [3];
    let unshiftOk = unshifted.unshift(mark('w', 1), mark('x', 2)) === 3 &&
        unshifted.join('|') === '1|2|3';
    let concatOk = [1].concat(mark('y', [2]), mark('z', 3)).join('|') === '1|2|3';
    let arrayOk = Array(mark('A', 4), mark('B', 5)).join('|') === '4|5';

    return joinOk &&
        includesOk &&
        indexOfOk &&
        lastIndexOfOk &&
        sliceOk &&
        reverseOk &&
        popOk &&
        shiftOk &&
        pushOk &&
        unshiftOk &&
        concatOk &&
        arrayOk;
};

let first = run();
let second = run();

first &&
    second &&
    order === 'abcdefghijklmnopqrstuvwxyzABabcdefghijklmnopqrstuvwxyzAB' ? 42 : 0
";

#[test]
fn direct_array_targets_preserve_argument_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(ARRAY_NATIVE_ARGUMENT_FAST_PATH_SCRIPT)?;
    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        11,
        "direct Array native call operands",
    )?;
    ensure_at_least(
        script.usage().bytecode_array_native_call_count(),
        1,
        "direct Array constructor operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let usage = vm.resource_usage();
    ensure_at_least(
        usage.native_call_cache_misses,
        11,
        "direct Array native call cache misses",
    )
}

#[test]
fn array_method_arguments_run_before_receiver_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval(
        r"
        let object = {};
        object.join = Array.prototype.join;
        object.join(print('argument-before-error'));
        ",
    ) else {
        return Err("expected Array.prototype.join on a non-array receiver to fail".into());
    };

    ensure_error_contains(&error, "requires an array receiver")?;
    ensure_output(context.output(), &["argument-before-error".to_owned()])
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(error: &rs_quickjs::Error, text: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(text) {
        return Ok(());
    }

    Err(format!("expected error containing '{text}', got '{message}'").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }

    Err(format!("expected {label} >= {expected}, got {actual}").into())
}
