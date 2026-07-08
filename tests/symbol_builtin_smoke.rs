use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const SYMBOL_SCRIPT: &str = r#"
let first = Symbol("slot");
let second = Symbol("slot");
let object = {};
object[first] = 7;
object[second] = 9;

let descriptor = Object.getOwnPropertyDescriptor(object, first);
let keys = Object.keys(object);
let iteratorDescriptor = Object.getOwnPropertyDescriptor(Symbol, "iterator");
let descriptionDescriptor = Object.getOwnPropertyDescriptor(Symbol.prototype, "description");
let tagged = {};
tagged[Symbol.toStringTag] = "tagged";
let boxed = Object(first);
let emptyDescription = Symbol();

typeof Symbol === "function" &&
    Symbol.name === "Symbol" &&
    Symbol.length === 0 &&
    typeof first === "symbol" &&
    String(first) === "Symbol(slot)" &&
    first !== second &&
    first.description === "slot" &&
    emptyDescription.description === undefined &&
    first.toString() === "Symbol(slot)" &&
    boxed.toString() === "Symbol(slot)" &&
    first.valueOf() === first &&
    boxed.valueOf() === first &&
    Object(first).valueOf() === first &&
    object[first] === 7 &&
    object[second] === 9 &&
    Object.hasOwn(object, first) === true &&
    Object.hasOwn(object, second) === true &&
    descriptor.value === 7 &&
    descriptor.enumerable === true &&
    descriptor.writable === true &&
    descriptor.configurable === true &&
    keys.length === 0 &&
    typeof Symbol.iterator === "symbol" &&
    Symbol.iterator === Symbol.iterator &&
    Symbol.iterator !== Symbol.toStringTag &&
    iteratorDescriptor.value === Symbol.iterator &&
    iteratorDescriptor.enumerable === false &&
    iteratorDescriptor.writable === false &&
    iteratorDescriptor.configurable === false &&
    typeof descriptionDescriptor.get === "function" &&
    descriptionDescriptor.set === undefined &&
    descriptionDescriptor.enumerable === false &&
    descriptionDescriptor.configurable === true &&
    tagged[Symbol.toStringTag] === "tagged" ? 42 : 0
"#;

#[test]
fn supports_symbol_primitives_and_property_keys() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(SYMBOL_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_symbol_prototype_value_methods_for_wrong_receivers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let error = context.eval("Symbol.prototype.valueOf.call({})");
    let Err(error) = error else {
        return Err("expected Symbol.prototype.valueOf to reject ordinary object receiver".into());
    };
    let text = error.to_string();
    if text.contains("Symbol.prototype value method") {
        return Ok(());
    }

    Err(format!("unexpected error: {text}").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
