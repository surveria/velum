use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const OBJECT_DESCRIPTOR_SCRIPT: &str = r#"
let objectKeys = "";
for (let key in Object) {
    objectKeys = objectKeys + key + ";";
}

let object = { a: 1 };
let returned = Object.defineProperty(object, "hidden", { value: 9 });
Object.defineProperty(object, "fixed", {
    value: 7,
    enumerable: true,
    writable: false,
    configurable: false
});
Object.defineProperty(object, "open", {
    value: 3,
    enumerable: true,
    writable: true,
    configurable: true
});

let fixedDescriptor = Object.getOwnPropertyDescriptor(object, "fixed");
let hiddenDescriptor = Object.getOwnPropertyDescriptor(object, "hidden");
let missingDescriptor = Object.getOwnPropertyDescriptor(object, "missing");
object.fixed = 8;
let deleteFixed = delete object.fixed;
let deleteHidden = delete object.hidden;
let deleteOpen = delete object.open;
let keys = Object.keys(object);
let child = { __proto__: object, own: 5 };

print(
    typeof Object.getOwnPropertyDescriptor,
    Object.getOwnPropertyDescriptor.name,
    Object.getOwnPropertyDescriptor.length,
    typeof Object.defineProperty,
    Object.defineProperty.name,
    Object.defineProperty.length,
    typeof Object.keys,
    Object.keys.name,
    Object.keys.length,
    typeof Object.hasOwn,
    Object.hasOwn.name,
    Object.hasOwn.length
);
print(
    fixedDescriptor.value,
    fixedDescriptor.enumerable,
    fixedDescriptor.writable,
    fixedDescriptor.configurable,
    object.fixed,
    deleteFixed
);
print(
    hiddenDescriptor.value,
    hiddenDescriptor.enumerable,
    hiddenDescriptor.writable,
    hiddenDescriptor.configurable,
    deleteHidden,
    missingDescriptor
);
print(
    keys.length,
    keys[0],
    keys[1],
    Object.hasOwn(object, "fixed"),
    Object.hasOwn(child, "fixed"),
    "fixed" in child,
    Object.hasOwn(child, "own"),
    deleteOpen,
    "keys:" + objectKeys
);

returned === object &&
    typeof Object.getOwnPropertyDescriptor === "function" &&
    Object.getOwnPropertyDescriptor.name === "getOwnPropertyDescriptor" &&
    Object.getOwnPropertyDescriptor.length === 2 &&
    typeof Object.defineProperty === "function" &&
    Object.defineProperty.name === "defineProperty" &&
    Object.defineProperty.length === 3 &&
    typeof Object.keys === "function" &&
    Object.keys.name === "keys" &&
    Object.keys.length === 1 &&
    typeof Object.hasOwn === "function" &&
    Object.hasOwn.name === "hasOwn" &&
    Object.hasOwn.length === 2 &&
    fixedDescriptor.value === 7 &&
    fixedDescriptor.enumerable === true &&
    fixedDescriptor.writable === false &&
    fixedDescriptor.configurable === false &&
    hiddenDescriptor.value === 9 &&
    hiddenDescriptor.enumerable === false &&
    hiddenDescriptor.writable === false &&
    hiddenDescriptor.configurable === false &&
    missingDescriptor === undefined &&
    object.fixed === 7 &&
    deleteFixed === false &&
    deleteHidden === false &&
    deleteOpen === true &&
    keys.length === 2 &&
    keys[0] === "a" &&
    keys[1] === "fixed" &&
    Object.hasOwn(object, "fixed") === true &&
    Object.hasOwn(child, "fixed") === false &&
    "fixed" in child &&
    Object.hasOwn(child, "own") === true &&
    objectKeys === "" ? 42 : 0
"#;

#[test]
fn supports_data_property_descriptors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(OBJECT_DESCRIPTOR_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "function getOwnPropertyDescriptor 2 function defineProperty 3 function keys 1 function hasOwn 2",
            "7 true false false 7 false",
            "9 false false false false undefined",
            "2 a fixed true false true true true keys:",
        ],
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
