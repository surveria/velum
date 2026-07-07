use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const PRIMITIVE_CONSTRUCTOR_ARGUMENT_SCRIPT: &str = r#"
let order = "";

let mark = function(label, value) {
    order = order + label;
    return value;
};

let stringValueOk = String(mark("a", "front"), mark("b", "unused")) === "front";
let stringObject = new String(mark("c", "go"), mark("d", "unused"));
let numberValueOk = Number(mark("e", "7"), mark("f", "unused")) === 7;
let numberObject = new Number(mark("g", "8"), mark("h", "unused"));
let booleanFalseOk = Boolean(mark("i", 0), mark("j", 1)) === false;
let booleanObject = new Boolean(mark("k", ""), mark("l", true));
let objectValue = Object(mark("m", null), mark("n", "unused"));
let existing = { flag: 1 };
let objectSameOk = Object(mark("o", existing), mark("p", "unused")) === existing;

stringValueOk &&
    stringObject.length === 2 &&
    stringObject[0] === "g" &&
    typeof numberObject === "object" &&
    numberObject.__proto__ === Number.prototype &&
    booleanFalseOk &&
    typeof booleanObject === "object" &&
    booleanObject.__proto__ === Boolean.prototype &&
    Boolean(booleanObject) === true &&
    typeof objectValue === "object" &&
    objectSameOk &&
    order === "abcdefghijklmnop" ? 42 : 0
"#;

#[test]
fn primitive_constructor_calls_compile_to_direct_native_operands() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(PRIMITIVE_CONSTRUCTOR_ARGUMENT_SCRIPT)?;

    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        4,
        "primitive constructor direct native call operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn primitive_constructors_preserve_extra_argument_side_effects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(PRIMITIVE_CONSTRUCTOR_ARGUMENT_SCRIPT)?;

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
