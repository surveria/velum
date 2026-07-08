use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const INDEX_OF_SOURCE: &str = r#"
let values = [1, 2, 3, 2, undefined, null, "2"];
let firstTwo = values.indexOf(2);
let nextTwo = values.indexOf(2, 2);
let fromNegative = values.indexOf(2, -4);
let missing = values.indexOf(9);
let fromTooLarge = values.indexOf(1, 99);
let stringTwo = values.indexOf("2");
let undefinedIndex = values.indexOf(undefined);
let nullIndex = values.indexOf(null);
let stringStart = values.indexOf(2, "2");
let fractionStart = values.indexOf(2, 2.9);
let veryNegative = values.indexOf(1, -99);

let sparse = Array(3);
sparse[2] = "tail";
let holeUndefined = sparse.indexOf(undefined);
let tailIndex = sparse.indexOf("tail");
let tailFromEnd = sparse.indexOf("tail", -1);

let withUndefined = Array(2);
withUndefined[1] = undefined;
let ownUndefined = withUndefined.indexOf(undefined);

Array.prototype[1] = "proto-one";
let inherited = Array(3);
inherited[2] = "tail";
let inheritedIndex = inherited.indexOf("proto-one");
let inheritedUndefined = inherited.indexOf(undefined);
delete Array.prototype[1];

Array.prototype[0] = undefined;
let inheritedUndefinedValue = Array(1).indexOf(undefined);
delete Array.prototype[0];

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let extra = [7].indexOf(7, 0, marker());

let boolStart = [0, 1].indexOf(1, true);
let nullStart = [0].indexOf(0, null);
let missingSearch = [undefined].indexOf();

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("indexOf", firstTwo, nextTwo, fromNegative, missing, fromTooLarge, stringTwo);
print("values", undefinedIndex, nullIndex, stringStart, fractionStart, veryNegative);
print("sparse", holeUndefined, tailIndex, tailFromEnd, ownUndefined);
print("inherited", inheritedIndex, inheritedUndefined, inheritedUndefinedValue, side, extra);
print("coerced", boolStart, nullStart, missingSearch);
print("meta", typeof Array.prototype.indexOf, Array.prototype.indexOf.name, Array.prototype.indexOf.length);
print("keys:" + prototypeKeys);
print("in", "indexOf" in values);

firstTwo === 1 &&
    nextTwo === 3 &&
    fromNegative === 3 &&
    missing === -1 &&
    fromTooLarge === -1 &&
    stringTwo === 6 &&
    undefinedIndex === 4 &&
    nullIndex === 5 &&
    stringStart === 3 &&
    fractionStart === 3 &&
    veryNegative === 0 &&
    holeUndefined === -1 &&
    tailIndex === 2 &&
    tailFromEnd === 2 &&
    ownUndefined === 1 &&
    inheritedIndex === 1 &&
    inheritedUndefined === -1 &&
    inheritedUndefinedValue === 0 &&
    side === 42 &&
    extra === 0 &&
    boolStart === 1 &&
    nullStart === 0 &&
    missingSearch === 0 &&
    typeof Array.prototype.indexOf === "function" &&
    Array.prototype.indexOf.name === "indexOf" &&
    Array.prototype.indexOf.length === 1 &&
    prototypeKeys === "" &&
    ("indexOf" in values) ? 42 : 0
"#;

const INDEX_OF_OUTPUT: &[&str] = &[
    "indexOf 1 3 3 -1 -1 6",
    "values 4 5 3 3 0",
    "sparse -1 2 2 1",
    "inherited 1 -1 0 42 0",
    "coerced 1 0 0",
    "meta function indexOf 1",
    "keys:",
    "in true",
];

#[test]
fn supports_array_index_of_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(INDEX_OF_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), INDEX_OF_OUTPUT)
}

#[test]
fn supports_array_index_of_on_array_like_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { length: 4, 0: "a", 2: "a" };
        let first = Array.prototype.indexOf.call(object, "a");
        let second = Array.prototype.indexOf.call(object, "a", 1);
        let missing = Array.prototype.indexOf.call(object, undefined);
        first === 0 && second === 2 && missing === -1 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
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
