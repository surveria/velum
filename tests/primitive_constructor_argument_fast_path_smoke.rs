use rs_quickjs::{Runtime, Value};

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

stringValueOk &&
    stringObject.length === 2 &&
    stringObject[0] === "g" &&
    typeof numberObject === "object" &&
    numberObject.__proto__ === Number.prototype &&
    booleanFalseOk &&
    typeof booleanObject === "object" &&
    booleanObject.__proto__ === Boolean.prototype &&
    Boolean(booleanObject) === true &&
    order === "abcdefghijkl" ? 42 : 0
"#;

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
