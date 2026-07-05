use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const FUNCTION_INTRINSIC_DESCRIPTOR_SCRIPT: &str = r#"
let f = function namedCamera(a, b) {};
let initialNameDescriptor = Object.getOwnPropertyDescriptor(f, "name");
let initialLengthDescriptor = Object.getOwnPropertyDescriptor(f, "length");

let deletedName = delete f.name;
let hasNameAfterDelete = Object.hasOwn(f, "name");
let nameDescriptorAfterDelete = Object.getOwnPropertyDescriptor(f, "name");

Object.defineProperty(f, "name", {
    value: "patched",
    writable: true,
    enumerable: true,
    configurable: true
});
f.name = "assigned";
let keysAfterName = Object.keys(f);
let nameDescriptorAfterAssign = Object.getOwnPropertyDescriptor(f, "name");

Object.defineProperty(f, "length", {
    value: 5,
    writable: true,
    configurable: true
});
f.length = 7;
let deletedLength = delete f.length;
let hasLengthAfterDelete = Object.hasOwn(f, "length");
let lengthDescriptorAfterDelete = Object.getOwnPropertyDescriptor(f, "length");

Object.defineProperty(f, "length", {
    value: 11,
    writable: true,
    enumerable: true,
    configurable: true
});
let keysAfterLength = Object.keys(f);
let lengthDescriptorAfterAssign = Object.getOwnPropertyDescriptor(f, "length");

let nativeNameDescriptor = Object.getOwnPropertyDescriptor(TypeError, "name");
let nativeLengthDescriptor = Object.getOwnPropertyDescriptor(TypeError, "length");
Object.defineProperty(TypeError, "name", {
    value: "Typed",
    writable: true,
    configurable: true
});
TypeError.name = "TypedAssigned";
let nativeName = TypeError.name;
let deletedNativeName = delete TypeError.name;
let nativeHasNameAfterDelete = Object.hasOwn(TypeError, "name");

Object.defineProperty(TypeError, "length", {
    value: 4,
    writable: true,
    configurable: true
});
TypeError.length = 6;
let nativeLength = TypeError.length;
let deletedNativeLength = delete TypeError.length;
let nativeHasLengthAfterDelete = Object.hasOwn(TypeError, "length");

print(
    initialNameDescriptor.value,
    initialNameDescriptor.writable,
    initialNameDescriptor.enumerable,
    initialNameDescriptor.configurable,
    initialLengthDescriptor.value,
    initialLengthDescriptor.writable,
    initialLengthDescriptor.enumerable,
    initialLengthDescriptor.configurable
);
print(
    deletedName,
    hasNameAfterDelete,
    nameDescriptorAfterDelete,
    f.name,
    keysAfterName.length,
    keysAfterName[0],
    nameDescriptorAfterAssign.writable,
    nameDescriptorAfterAssign.enumerable,
    nameDescriptorAfterAssign.configurable
);
print(
    f.length,
    deletedLength,
    hasLengthAfterDelete,
    lengthDescriptorAfterDelete,
    lengthDescriptorAfterAssign.value,
    lengthDescriptorAfterAssign.writable,
    lengthDescriptorAfterAssign.enumerable,
    lengthDescriptorAfterAssign.configurable,
    keysAfterLength.length,
    keysAfterLength[1]
);
print(
    nativeNameDescriptor.value,
    nativeLengthDescriptor.value,
    nativeName,
    deletedNativeName,
    nativeHasNameAfterDelete,
    nativeLength,
    deletedNativeLength,
    nativeHasLengthAfterDelete
);

initialNameDescriptor.value === "namedCamera" &&
    initialNameDescriptor.writable === false &&
    initialNameDescriptor.enumerable === false &&
    initialNameDescriptor.configurable === true &&
    initialLengthDescriptor.value === 2 &&
    initialLengthDescriptor.writable === false &&
    initialLengthDescriptor.enumerable === false &&
    initialLengthDescriptor.configurable === true &&
    deletedName === true &&
    hasNameAfterDelete === false &&
    nameDescriptorAfterDelete === undefined &&
    f.name === "assigned" &&
    keysAfterName.length === 1 &&
    keysAfterName[0] === "name" &&
    nameDescriptorAfterAssign.writable === true &&
    nameDescriptorAfterAssign.enumerable === true &&
    nameDescriptorAfterAssign.configurable === true &&
    f.length === 11 &&
    deletedLength === true &&
    hasLengthAfterDelete === false &&
    lengthDescriptorAfterDelete === undefined &&
    lengthDescriptorAfterAssign.value === 11 &&
    lengthDescriptorAfterAssign.writable === true &&
    lengthDescriptorAfterAssign.enumerable === true &&
    lengthDescriptorAfterAssign.configurable === true &&
    keysAfterLength.length === 2 &&
    keysAfterLength[1] === "length" &&
    nativeNameDescriptor.value === "TypeError" &&
    nativeLengthDescriptor.value === 1 &&
    nativeName === "TypedAssigned" &&
    deletedNativeName === true &&
    nativeHasNameAfterDelete === false &&
    nativeLength === 6 &&
    deletedNativeLength === true &&
    nativeHasLengthAfterDelete === false ? 42 : 0
"#;

#[test]
fn supports_function_intrinsic_descriptor_reconfiguration() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(FUNCTION_INTRINSIC_DESCRIPTOR_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "namedCamera false false true 2 false false true",
            "true false undefined assigned 1 name true true true",
            "11 true false undefined 11 true true true 2 length",
            "TypeError 1 TypedAssigned true false 6 true false",
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
