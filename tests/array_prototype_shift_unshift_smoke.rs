use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const SHIFT_UNSHIFT_SOURCE: &str = r#"
let values = [1, 2, 3];
let first = values.shift();

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
[9].shift(marker());

let sparse = Array(3);
sparse[2] = "tail";
let sparseFirst = sparse.shift();

Array.prototype[1] = "proto-one";
let inheritedShift = Array(2);
let inheritedShiftFirst = inheritedShift.shift();
let inheritedShiftValue = inheritedShift[0];
delete Array.prototype[1];

let base = [3];
let newLength = base.unshift(1, 2);
let sameLength = base.unshift();

let sparseUnshift = Array(2);
sparseUnshift[1] = "b";
let sparseLength = sparseUnshift.unshift("a");

Array.prototype[0] = "proto-zero";
let inheritedUnshift = Array(1);
let inheritedUnshiftLength = inheritedUnshift.unshift("head");
let inheritedUnshiftJoin = inheritedUnshift.join("|");
delete Array.prototype[0];

let emptyShift = [].shift();

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("shift", first, values.length, values[0], values[1], values[2], side);
print("sparse", sparseFirst, sparse.length, "0" in sparse, sparse[0], sparse[1]);
print("inherited", inheritedShiftFirst, inheritedShift.length, inheritedShiftValue);
print("unshift", newLength, sameLength, base.length, base[0], base[1], base[2]);
print("holes", sparseLength, "1" in sparseUnshift, sparseUnshift.join("|"));
print("inherited-unshift", inheritedUnshiftLength, inheritedUnshiftJoin, emptyShift);
print(
    "meta",
    typeof Array.prototype.shift,
    Array.prototype.shift.name,
    Array.prototype.shift.length,
    typeof Array.prototype.unshift,
    Array.prototype.unshift.name,
    Array.prototype.unshift.length
);
print("keys:" + prototypeKeys);
print("in", "shift" in base, "unshift" in base);

first === 1 &&
    values.length === 2 &&
    values[0] === 2 &&
    values[1] === 3 &&
    values[2] === undefined &&
    side === 42 &&
    sparseFirst === undefined &&
    sparse.length === 2 &&
    !("0" in sparse) &&
    sparse[0] === undefined &&
    sparse[1] === "tail" &&
    inheritedShiftFirst === undefined &&
    inheritedShift.length === 1 &&
    inheritedShiftValue === "proto-one" &&
    newLength === 3 &&
    sameLength === 3 &&
    base.length === 3 &&
    base[0] === 1 &&
    base[1] === 2 &&
    base[2] === 3 &&
    sparseLength === 3 &&
    !("1" in sparseUnshift) &&
    sparseUnshift.join("|") === "a||b" &&
    inheritedUnshiftLength === 2 &&
    inheritedUnshiftJoin === "head|proto-zero" &&
    emptyShift === undefined &&
    typeof Array.prototype.shift === "function" &&
    Array.prototype.shift.name === "shift" &&
    Array.prototype.shift.length === 0 &&
    typeof Array.prototype.unshift === "function" &&
    Array.prototype.unshift.name === "unshift" &&
    Array.prototype.unshift.length === 1 &&
    prototypeKeys === "" &&
    ("shift" in base) &&
    ("unshift" in base) ? 42 : 0
"#;

const SHIFT_UNSHIFT_OUTPUT: &[&str] = &[
    "shift 1 2 2 3 undefined 42",
    "sparse undefined 2 false undefined tail",
    "inherited undefined 1 proto-one",
    "unshift 3 3 3 1 2 3",
    "holes 3 false a||b",
    "inherited-unshift 2 head|proto-zero undefined",
    "meta function shift 0 function unshift 1",
    "keys:",
    "in true true",
];

#[test]
fn supports_array_shift_and_unshift_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(SHIFT_UNSHIFT_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), SHIFT_UNSHIFT_OUTPUT)
}

#[test]
fn rejects_shift_and_unshift_on_non_array_receivers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(shift_error) = context.eval(
        r"
        let object = {};
        object.shift = Array.prototype.shift;
        object.shift();
        ",
    ) else {
        return Err("expected Array.prototype.shift on non-array receiver to fail".into());
    };
    ensure_error_contains(&shift_error, "requires an array receiver")?;

    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(unshift_error) = context.eval(
        r"
        let object = {};
        object.unshift = Array.prototype.unshift;
        object.unshift(1);
        ",
    ) else {
        return Err("expected Array.prototype.unshift on non-array receiver to fail".into());
    };
    ensure_error_contains(&unshift_error, "requires an array receiver")
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
