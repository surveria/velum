use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const INCLUDES_SOURCE: &str = r#"
let nan = 0 / 0;
let values = [1, 2, 3, 2, undefined, null, "2", nan, -0];
let hasTwo = values.includes(2);
let nextTwo = values.includes(2, 2);
let lateTwo = values.includes(2, 4);
let missing = values.includes(9);
let fromNegative = values.includes(null, -4);
let stringTwo = values.includes("2");
let undefinedMatch = values.includes(undefined);
let nullMatch = values.includes(null);
let stringStart = values.includes(2, "2");
let fractionStart = values.includes(2, 2.9);
let veryNegative = values.includes(1, -99);
let nanMatch = values.includes(0 / 0);
let zeroMatch = values.includes(0);
let fromTooLarge = values.includes(1, 99);

let sparse = Array(3);
sparse[2] = "tail";
let holeUndefined = sparse.includes(undefined);
let tailIndex = sparse.includes("tail");
let tailFromEnd = sparse.includes("tail", -1);
let sparseMissing = sparse.includes("missing");

let withUndefined = Array(2);
withUndefined[1] = undefined;
let ownUndefined = withUndefined.includes(undefined);

Array.prototype[1] = "proto-one";
let inherited = Array(3);
inherited[2] = "tail";
let inheritedMatch = inherited.includes("proto-one");
let inheritedUndefined = inherited.includes(undefined);
delete Array.prototype[1];

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let extra = [7].includes(7, 0, marker());

let boolStart = [0, 1].includes(1, true);
let nullStart = [0].includes(0, null);
let missingSearch = [undefined].includes();

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("includes", hasTwo, nextTwo, lateTwo, missing, fromNegative, stringTwo);
print("values", undefinedMatch, nullMatch, stringStart, fractionStart, veryNegative, nanMatch, zeroMatch, fromTooLarge);
print("sparse", holeUndefined, tailIndex, tailFromEnd, ownUndefined, sparseMissing);
print("inherited", inheritedMatch, inheritedUndefined, side, extra);
print("coerced", boolStart, nullStart, missingSearch);
print("meta", typeof Array.prototype.includes, Array.prototype.includes.name, Array.prototype.includes.length);
print("keys:" + prototypeKeys);
print("in", "includes" in values);

hasTwo &&
    nextTwo &&
    !lateTwo &&
    !missing &&
    fromNegative &&
    stringTwo &&
    undefinedMatch &&
    nullMatch &&
    stringStart &&
    fractionStart &&
    veryNegative &&
    nanMatch &&
    zeroMatch &&
    !fromTooLarge &&
    holeUndefined &&
    tailIndex &&
    tailFromEnd &&
    ownUndefined &&
    !sparseMissing &&
    inheritedMatch &&
    inheritedUndefined &&
    side === 42 &&
    extra &&
    boolStart &&
    nullStart &&
    missingSearch &&
    typeof Array.prototype.includes === "function" &&
    Array.prototype.includes.name === "includes" &&
    Array.prototype.includes.length === 1 &&
    prototypeKeys === "" &&
    ("includes" in values) ? 42 : 0
"#;

const INCLUDES_OUTPUT: &[&str] = &[
    "includes true true false false true true",
    "values true true true true true true true false",
    "sparse true true true true false",
    "inherited true true 42 true",
    "coerced true true true",
    "meta function includes 1",
    "keys:",
    "in true",
];

#[test]
fn supports_array_includes_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(INCLUDES_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), INCLUDES_OUTPUT)
}

#[test]
fn supports_array_includes_on_array_like_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { length: 4, 0: "a", 2: NaN };
        Array.prototype.includes.call(object, "a") &&
            Array.prototype.includes.call(object, undefined, 1) &&
            Array.prototype.includes.call(object, NaN) &&
            !Array.prototype.includes.call(object, "missing") ? 42 : 0
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
