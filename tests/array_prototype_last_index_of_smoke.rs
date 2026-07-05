use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const LAST_INDEX_OF_SOURCE: &str = r#"
let values = [1, 2, 3, 2, undefined, null, "2"];
let lastTwo = values.lastIndexOf(2);
let beforeLast = values.lastIndexOf(2, 2);
let fromNegative = values.lastIndexOf(2, -4);
let missing = values.lastIndexOf(9);
let fromTooLarge = values.lastIndexOf(1, 99);
let stringTwo = values.lastIndexOf("2");
let undefinedIndex = values.lastIndexOf(undefined);
let nullIndex = values.lastIndexOf(null);
let stringStart = values.lastIndexOf(2, "2");
let fractionStart = values.lastIndexOf(2, 2.9);
let veryNegative = values.lastIndexOf(1, -99);
let undefinedStart = values.lastIndexOf(2, undefined);

let sparse = Array(3);
sparse[2] = "tail";
let holeUndefined = sparse.lastIndexOf(undefined);
let tailIndex = sparse.lastIndexOf("tail");
let tailBeforeEnd = sparse.lastIndexOf("tail", 1);
let tailFromEnd = sparse.lastIndexOf("tail", -1);

let withUndefined = Array(2);
withUndefined[1] = undefined;
let ownUndefined = withUndefined.lastIndexOf(undefined);

Array.prototype[1] = "proto-one";
let inherited = Array(3);
inherited[2] = "tail";
let inheritedIndex = inherited.lastIndexOf("proto-one");
let inheritedUndefined = inherited.lastIndexOf(undefined);
delete Array.prototype[1];

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let extra = [7].lastIndexOf(7, 0, marker());

let boolStart = [0, 1].lastIndexOf(1, true);
let nullStart = [0].lastIndexOf(0, null);
let missingSearch = [undefined].lastIndexOf();

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("lastIndexOf", lastTwo, beforeLast, fromNegative, missing, fromTooLarge, stringTwo);
print("values", undefinedIndex, nullIndex, stringStart, fractionStart, veryNegative, undefinedStart);
print("sparse", holeUndefined, tailIndex, tailBeforeEnd, tailFromEnd, ownUndefined);
print("inherited", inheritedIndex, inheritedUndefined, side, extra);
print("coerced", boolStart, nullStart, missingSearch);
print("meta", typeof Array.prototype.lastIndexOf, Array.prototype.lastIndexOf.name, Array.prototype.lastIndexOf.length);
print("keys:" + prototypeKeys);
print("in", "lastIndexOf" in values);

lastTwo === 3 &&
    beforeLast === 1 &&
    fromNegative === 3 &&
    missing === -1 &&
    fromTooLarge === 0 &&
    stringTwo === 6 &&
    undefinedIndex === 4 &&
    nullIndex === 5 &&
    stringStart === 1 &&
    fractionStart === 1 &&
    veryNegative === -1 &&
    undefinedStart === -1 &&
    holeUndefined === -1 &&
    tailIndex === 2 &&
    tailBeforeEnd === -1 &&
    tailFromEnd === 2 &&
    ownUndefined === 1 &&
    inheritedIndex === 1 &&
    inheritedUndefined === -1 &&
    side === 42 &&
    extra === 0 &&
    boolStart === 1 &&
    nullStart === 0 &&
    missingSearch === 0 &&
    typeof Array.prototype.lastIndexOf === "function" &&
    Array.prototype.lastIndexOf.name === "lastIndexOf" &&
    Array.prototype.lastIndexOf.length === 1 &&
    prototypeKeys === "" &&
    ("lastIndexOf" in values) ? 42 : 0
"#;

const LAST_INDEX_OF_OUTPUT: &[&str] = &[
    "lastIndexOf 3 1 3 -1 0 6",
    "values 4 5 1 1 -1 -1",
    "sparse -1 2 -1 2 1",
    "inherited 1 -1 42 0",
    "coerced 1 0 0",
    "meta function lastIndexOf 1",
    "keys:",
    "in true",
];

#[test]
fn supports_array_last_index_of_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(LAST_INDEX_OF_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), LAST_INDEX_OF_OUTPUT)
}

#[test]
fn rejects_array_last_index_of_on_non_array_receiver() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval(
        r"
        let object = {};
        object.lastIndexOf = Array.prototype.lastIndexOf;
        object.lastIndexOf(1);
        ",
    ) else {
        return Err("expected Array.prototype.lastIndexOf on non-array receiver to fail".into());
    };
    ensure_error_contains(&error, "requires an array receiver")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    if actual.len() != expected.len() {
        return Err(format!("expected output {expected:?}, got {actual:?}").into());
    }

    for (actual_line, expected_line) in actual.iter().zip(expected.iter()) {
        if actual_line != expected_line {
            return Err(format!("expected output {expected:?}, got {actual:?}").into());
        }
    }
    Ok(())
}

fn ensure_error_contains(error: &rs_quickjs::Error, text: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(text) {
        return Ok(());
    }

    Err(format!("expected error containing '{text}', got '{message}'").into())
}
