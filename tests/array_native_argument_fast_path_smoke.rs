use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const ARRAY_NATIVE_ARGUMENT_FAST_PATH_SCRIPT: &str = r"
let order = '';
let mark = function(label, value) {
    order = order + label;
    return value;
};

let joinOk = [1, 2].join(mark('a', '-'), mark('b', 'unused')) === '1-2';
let includesOk = [1, 2].includes(mark('c', 2), mark('d', 0), mark('e', 99)) === true;
let indexOfOk = [1, 2].indexOf(mark('f', 2), mark('g', 0), mark('h', 99)) === 1;
let lastIndexOfOk = [1, 2, 2].lastIndexOf(mark('i', 2), mark('j', 1), mark('k', 99)) === 1;
let sliceOk = [1, 2, 3].slice(mark('l', 1), mark('m', 3), mark('n', 99)).join('|') === '2|3';

let reversed = [1, 2];
let reverseOk = reversed.reverse(mark('o', 0), mark('p', 0)).join('|') === '2|1';
let popOk = [7].pop(mark('q', 0), mark('r', 0)) === 7;
let shiftOk = [8].shift(mark('s', 0), mark('t', 0)) === 8;

joinOk &&
    includesOk &&
    indexOfOk &&
    lastIndexOfOk &&
    sliceOk &&
    reverseOk &&
    popOk &&
    shiftOk &&
    order === 'abcdefghijklmnopqrst' ? 42 : 0
";

#[test]
fn fixed_arity_array_methods_preserve_extra_argument_side_effects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(ARRAY_NATIVE_ARGUMENT_FAST_PATH_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
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
