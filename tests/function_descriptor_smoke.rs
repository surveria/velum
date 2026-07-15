use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const FUNCTION_DESCRIPTOR_SCRIPT: &str = r#"
let f = function namedCamera(a, b) {};
Object.defineProperty(f, "tag", {
    value: "camera",
    enumerable: true,
    writable: false,
    configurable: false
});
Object.defineProperty(f, "hidden", { value: 9 });
f.tag = "changed";
let deleteTag = delete f.tag;
let tagDescriptor = Object.getOwnPropertyDescriptor(f, "tag");
let hiddenDescriptor = Object.getOwnPropertyDescriptor(f, "hidden");
let nameDescriptor = Object.getOwnPropertyDescriptor(f, "name");
let lengthDescriptor = Object.getOwnPropertyDescriptor(f, "length");
let functionKeys = Object.keys(f);

Object.defineProperty(Object.keys, "tag", {
    value: "native",
    enumerable: true,
    writable: false,
    configurable: false
});
Object.keys.tag = "changed";
let deleteNativeTag = delete Object.keys.tag;
let nativeTagDescriptor = Object.getOwnPropertyDescriptor(Object.keys, "tag");
let nativeNameDescriptor = Object.getOwnPropertyDescriptor(Object.keys, "name");
let nativeLengthDescriptor = Object.getOwnPropertyDescriptor(Object.keys, "length");
let nativeKeys = Object.keys(Object.keys);

print(
    f.tag,
    functionKeys.length,
    functionKeys[0],
    tagDescriptor.enumerable,
    tagDescriptor.writable,
    tagDescriptor.configurable,
    deleteTag
);
print(
    hiddenDescriptor.value,
    hiddenDescriptor.enumerable,
    nameDescriptor.value,
    nameDescriptor.configurable,
    lengthDescriptor.value,
    lengthDescriptor.configurable
);
print(
    Object.keys.tag,
    nativeKeys.length,
    nativeKeys[0],
    nativeTagDescriptor.enumerable,
    nativeTagDescriptor.writable,
    nativeTagDescriptor.configurable,
    deleteNativeTag
);
print(
    nativeNameDescriptor.value,
    nativeNameDescriptor.configurable,
    nativeLengthDescriptor.value,
    nativeLengthDescriptor.configurable
);

f.tag === "camera" &&
    functionKeys.length === 1 &&
    functionKeys[0] === "tag" &&
    tagDescriptor.enumerable === true &&
    tagDescriptor.writable === false &&
    tagDescriptor.configurable === false &&
    deleteTag === false &&
    hiddenDescriptor.value === 9 &&
    hiddenDescriptor.enumerable === false &&
    nameDescriptor.value === "namedCamera" &&
    nameDescriptor.configurable === true &&
    lengthDescriptor.value === 2 &&
    lengthDescriptor.configurable === true &&
    Object.keys.tag === "native" &&
    nativeKeys.length === 1 &&
    nativeKeys[0] === "tag" &&
    nativeTagDescriptor.enumerable === true &&
    nativeTagDescriptor.writable === false &&
    nativeTagDescriptor.configurable === false &&
    deleteNativeTag === false &&
    nativeNameDescriptor.value === "keys" &&
    nativeNameDescriptor.configurable === true &&
    nativeLengthDescriptor.value === 1 &&
    nativeLengthDescriptor.configurable === true ? 42 : 0
"#;

#[test]
fn supports_function_data_property_descriptors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(FUNCTION_DESCRIPTOR_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "camera 1 tag true false false false",
            "9 false namedCamera true 2 true",
            "native 1 tag true false false false",
            "keys true 1 true",
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
