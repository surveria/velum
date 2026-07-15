use velum::{Runtime, Value};

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
fn supports_shift_and_unshift_on_array_like_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let shifted = { length: 3, 0: "a", 2: "c" };
        let first = Array.prototype.shift.call(shifted);
        let unshifted = { length: 2, 1: "tail" };
        let newLength = Array.prototype.unshift.call(unshifted, "head");
        first === "a" &&
            shifted.length === 2 &&
            !("0" in shifted) &&
            shifted[1] === "c" &&
            !("2" in shifted) &&
            newLength === 3 &&
            unshifted.length === 3 &&
            unshifted[0] === "head" &&
            !("1" in unshifted) &&
            unshifted[2] === "tail" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_shift_and_unshift_at_non_writable_indices() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let shifted = [1, 2, 3];
        Object.defineProperty(shifted, "0", {
            value: 1,
            writable: false,
            enumerable: true,
            configurable: true
        });
        let shiftTypeError = false;
        try {
            shifted.shift();
        } catch (error) {
            shiftTypeError = error instanceof TypeError;
        }

        let unshifted = [3];
        Object.defineProperty(unshifted, "0", {
            value: 3,
            writable: false,
            enumerable: true,
            configurable: true
        });
        let unshiftTypeError = false;
        try {
            unshifted.unshift(1, 2);
        } catch (error) {
            unshiftTypeError = error instanceof TypeError;
        }

        shiftTypeError &&
            shifted.length === 3 &&
            shifted[0] === 1 &&
            shifted[1] === 2 &&
            shifted[2] === 3 &&
            unshiftTypeError &&
            unshifted.length === 3 &&
            unshifted[0] === 3 &&
            unshifted[1] === undefined &&
            unshifted[2] === 3 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_shift_and_unshift_at_non_configurable_indices() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let shifted = Array(3);
        Object.defineProperty(shifted, "0", {
            value: "zero",
            writable: true,
            enumerable: true,
            configurable: false
        });
        shifted[2] = "tail";
        let shiftTypeError = false;
        try {
            shifted.shift();
        } catch (error) {
            shiftTypeError = error instanceof TypeError;
        }

        let unshifted = Array(2);
        Object.defineProperty(unshifted, "1", {
            value: "tail",
            writable: true,
            enumerable: true,
            configurable: false
        });
        let unshiftTypeError = false;
        try {
            unshifted.unshift("head");
        } catch (error) {
            unshiftTypeError = error instanceof TypeError;
        }

        shiftTypeError &&
            shifted.length === 3 &&
            shifted[0] === "zero" &&
            shifted[1] === undefined &&
            shifted[2] === "tail" &&
            unshiftTypeError &&
            unshifted.length === 3 &&
            unshifted[0] === undefined &&
            unshifted[1] === "tail" &&
            unshifted[2] === "tail" ? 42 : 0
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
