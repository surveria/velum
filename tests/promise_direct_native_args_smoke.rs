use velum::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const PROMISE_DIRECT_TARGET_ARGS_SETUP: &str = r#"
var order = "";
var fulfilledTotal = 0;
var rejectedText = "";
var promiseCallThrows = 0;
var mark = function(label, value) {
    order = order + label;
    return value;
};
"#;

const PROMISE_DIRECT_TARGET_ARGS_SCRIPT: &str = r#"
try {
    Promise(mark("p", function() {}));
} catch (error) {
    if (error instanceof TypeError) {
        promiseCallThrows = promiseCallThrows + 1;
    }
}

var created = new Promise(function(resolve, reject) {
    order = order + "x";
    resolve(mark("a", 10));
});
created.then(function(value) {
    fulfilledTotal = fulfilledTotal + value;
});

var resolved = Promise.resolve(mark("b", 20));
resolved.then(mark("c", function(value) {
    fulfilledTotal = fulfilledTotal + value + 1;
}));

Promise.reject(mark("d", "offline")).catch(mark("e", function(reason) {
    rejectedText = rejectedText + reason + ";";
}));
"#;

const PROMISE_DIRECT_TARGET_ARGS_CHECK: &str = r#"
order === "pxabcdepxabcde" &&
    promiseCallThrows === 2 &&
    fulfilledTotal === 62 &&
    rejectedText === "offline;offline;" ? 42 : 0
"#;

#[test]
fn direct_promise_targets_preserve_argument_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(PROMISE_DIRECT_TARGET_ARGS_SETUP)?;
    let script = vm.compile(PROMISE_DIRECT_TARGET_ARGS_SCRIPT)?;

    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        5,
        "direct Promise native call operands",
    )?;

    vm.eval_compiled(&script)?;
    vm.eval_compiled(&script)?;

    let value = vm.eval(PROMISE_DIRECT_TARGET_ARGS_CHECK)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }

    Err(format!("expected {label} >= {expected}, got {actual}").into())
}
