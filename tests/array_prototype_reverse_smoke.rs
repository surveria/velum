use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const REVERSE_SOURCE: &str = r#"
let values = [1, 2, 3, 4];
let returned = values.reverse();
let sameObject = returned === values;

let odd = [1, 2, 3];
let oddReturned = odd.reverse();

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let sideCopy = [7];
let sideReturn = sideCopy.reverse(marker());

let sparse = Array(4);
sparse[1] = "one";
sparse[3] = "three";
let sparseReturn = sparse.reverse();

Array.prototype[2] = "proto-two";
let inheritedUpper = Array(3);
let inheritedUpperReturn = inheritedUpper.reverse();
delete Array.prototype[2];

Array.prototype[0] = "proto-zero";
let inheritedLower = Array(3);
let inheritedLowerReturn = inheritedLower.reverse();
delete Array.prototype[0];

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("reverse", sameObject, values.join("|"), values.length, oddReturned === odd, odd.join("|"));
print("side", side, sideReturn === sideCopy, sideCopy.join("|"));
print("sparse", sparse.length, sparse[0], "1" in sparse, sparse[2], "3" in sparse, sparse.join("|"), sparseReturn === sparse);
print("inherited-upper", inheritedUpperReturn === inheritedUpper, inheritedUpper[0], "0" in inheritedUpper, inheritedUpper[2], "2" in inheritedUpper);
print("inherited-lower", inheritedLowerReturn === inheritedLower, inheritedLower[0], "0" in inheritedLower, inheritedLower[2], "2" in inheritedLower);
print("meta", typeof Array.prototype.reverse, Array.prototype.reverse.name, Array.prototype.reverse.length);
print("keys:" + prototypeKeys);
print("in", "reverse" in values);

sameObject &&
    values.join("|") === "4|3|2|1" &&
    values.length === 4 &&
    oddReturned === odd &&
    odd.join("|") === "3|2|1" &&
    side === 42 &&
    sideReturn === sideCopy &&
    sideCopy.join("|") === "7" &&
    sparse.length === 4 &&
    sparse[0] === "three" &&
    !("1" in sparse) &&
    sparse[2] === "one" &&
    !("3" in sparse) &&
    sparse.join("|") === "three||one|" &&
    sparseReturn === sparse &&
    inheritedUpperReturn === inheritedUpper &&
    inheritedUpper[0] === "proto-two" &&
    ("0" in inheritedUpper) &&
    inheritedUpper[2] === undefined &&
    !("2" in inheritedUpper) &&
    inheritedLowerReturn === inheritedLower &&
    inheritedLower[0] === undefined &&
    !("0" in inheritedLower) &&
    inheritedLower[2] === "proto-zero" &&
    ("2" in inheritedLower) &&
    typeof Array.prototype.reverse === "function" &&
    Array.prototype.reverse.name === "reverse" &&
    Array.prototype.reverse.length === 0 &&
    prototypeKeys === "" &&
    ("reverse" in values) ? 42 : 0
"#;

const REVERSE_OUTPUT: &[&str] = &[
    "reverse true 4|3|2|1 4 true 3|2|1",
    "side 42 true 7",
    "sparse 4 three false one false three||one| true",
    "inherited-upper true proto-two true undefined false",
    "inherited-lower true undefined false proto-zero true",
    "meta function reverse 0",
    "keys:",
    "in true",
];

#[test]
fn supports_array_reverse_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(REVERSE_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), REVERSE_OUTPUT)
}

#[test]
fn keeps_descriptor_modified_arrays_on_generic_reverse_path() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [1, 2, 3];
        Object.defineProperty(values, "0", {
            value: 1,
            writable: false,
            enumerable: true,
            configurable: true
        });
        values.reverse();
        values[0] === 1 && values[1] === 2 && values[2] === 1 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn keeps_descriptor_modified_holey_arrays_on_generic_reverse_path() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = Array(3);
        Object.defineProperty(values, "0", {
            value: "zero",
            writable: true,
            enumerable: true,
            configurable: false
        });
        values.reverse();
        values[0] === "zero" &&
            values[2] === "zero" &&
            ("0" in values) &&
            ("2" in values) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_array_reverse_on_array_like_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { length: 4, 0: "a", 2: "c" };
        let returned = Array.prototype.reverse.call(object);
        returned === object &&
            object.length === 4 &&
            !("0" in object) &&
            object[1] === "c" &&
            !("2" in object) &&
            object[3] === "a" ? 42 : 0
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
