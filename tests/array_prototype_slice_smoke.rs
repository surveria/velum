use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const SLICE_SOURCE: &str = r#"
let values = [1, 2, 3, 4];
let middle = values.slice(1, 3);
let negative = values.slice(-3, -1);
let startOnly = values.slice(2);
let overflow = values.slice(99);
let reversed = values.slice(3, 1);

let sparse = Array(4);
sparse[1] = "one";
sparse[3] = "three";
let sparseCopy = sparse.slice(1, 4);

Array.prototype[1] = "proto-one";
let inherited = Array(3);
inherited[2] = "tail";
let inheritedCopy = inherited.slice(0, 3);
delete Array.prototype[1];

let side = 0;
let marker = function() {
    side = 42;
    return "ignored";
};
let sideCopy = [7].slice(0, 1, marker());

let coercedNull = [1, 2, 3].slice(null, "2");
let coercedBool = [1, 2, 3].slice(false, true);

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("slice", middle.join("|"), negative.join("|"), startOnly.join("|"), overflow.length, reversed.length);
print("source", values.length, values[0], values[1], values[2], values[3]);
print("sparse", sparseCopy.length, sparseCopy[0], "1" in sparseCopy, sparseCopy[1], sparseCopy[2], sparseCopy.join("|"), sparse.join("|"));
print("inherited", inheritedCopy.length, inheritedCopy[0], inheritedCopy[1], inheritedCopy[2], "1" in inheritedCopy);
print("coerced", coercedNull.join("|"), coercedBool.join("|"), side, sideCopy.join("|"));
print("meta", typeof Array.prototype.slice, Array.prototype.slice.name, Array.prototype.slice.length);
print("keys:" + prototypeKeys);
print("in", "slice" in values);

middle.join("|") === "2|3" &&
    negative.join("|") === "2|3" &&
    startOnly.join("|") === "3|4" &&
    overflow.length === 0 &&
    reversed.length === 0 &&
    values.length === 4 &&
    values[1] === 2 &&
    sparseCopy.length === 3 &&
    sparseCopy[0] === "one" &&
    !("1" in sparseCopy) &&
    sparseCopy[1] === undefined &&
    sparseCopy[2] === "three" &&
    sparse.join("|") === "|one||three" &&
    inheritedCopy.length === 3 &&
    inheritedCopy[0] === undefined &&
    inheritedCopy[1] === "proto-one" &&
    inheritedCopy[2] === "tail" &&
    ("1" in inheritedCopy) &&
    coercedNull.join("|") === "1|2" &&
    coercedBool.join("|") === "1" &&
    side === 42 &&
    sideCopy.join("|") === "7" &&
    typeof Array.prototype.slice === "function" &&
    Array.prototype.slice.name === "slice" &&
    Array.prototype.slice.length === 2 &&
    prototypeKeys === "" &&
    ("slice" in values) ? 42 : 0
"#;

const SLICE_OUTPUT: &[&str] = &[
    "slice 2|3 2|3 3|4 0 0",
    "source 4 1 2 3 4",
    "sparse 3 one false undefined three one||three |one||three",
    "inherited 3 undefined proto-one tail true",
    "coerced 1|2 1 42 7",
    "meta function slice 2",
    "keys:",
    "in true",
];

#[test]
fn supports_array_slice_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(SLICE_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), SLICE_OUTPUT)
}

#[test]
fn supports_array_slice_on_array_like_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { length: 4, 0: "a", 2: "c", 3: "d" };
        let sliced = Array.prototype.slice.call(object, 1, 4);
        sliced.length === 3 &&
            !("0" in sliced) &&
            sliced[1] === "c" &&
            sliced[2] === "d" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn slice_honors_species_order_sparse_results_and_data_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let log = "";
        let values = [1, , 3, 4];
        Object.defineProperty(values, "constructor", {
            get: function() {
                log = log + "constructor;";
                return {
                    get [Symbol.species]() {
                        log = log + "species;";
                        return function Result(length) {
                            log = log + "construct:" + length + ";";
                            return { kind: "slice" };
                        };
                    }
                };
            }
        });
        let start = { valueOf: function() { log = log + "start;"; return 1; } };
        let end = { valueOf: function() { log = log + "end;"; return 4; } };

        let result = values.slice(start, end);
        let descriptor = Object.getOwnPropertyDescriptor(result, "1");

        log === "start;end;constructor;species;construct:3;" &&
            result.kind === "slice" &&
            result.length === 3 &&
            !("0" in result) &&
            result[1] === 3 && result[2] === 4 &&
            descriptor.writable && descriptor.enumerable && descriptor.configurable ? 42 : 0
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
