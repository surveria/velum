use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const LOGICAL_ASSIGNMENT_SOURCE: &str = r#"
let order = "";
let mark = function(label, value) {
    order += label;
    return value;
};

let followsNull = mark("a", null) ?? mark("b", 7);
let keepsZero = mark("c", 0) ?? mark("d", 9);
let keepsFalse = mark("e", false) ?? mark("f", true);
let followsUndefined = mark("g", undefined) ?? mark("h", "fallback");
print(order, followsNull, keepsZero, keepsFalse, followsUndefined);

let truthy = 1;
let falsy = 0;
let empty = null;
let missing = undefined;
let andValue = truthy &&= 6;
let skippedAnd = falsy &&= mark("i", 99);
let orValue = falsy ||= 5;
let skippedOr = truthy ||= mark("j", 88);
let nullishValue = empty ??= 4;
let undefinedValue = missing ??= 3;
print(andValue, skippedAnd, orValue, skippedOr, nullishValue, undefinedValue);

let target = { slot: 0, keep: "value", empty: null, yes: true, no: false };
let key = function(name) {
    order += "k" + name + ";";
    return name;
};
let rhs = function(label, value) {
    order += "r" + label + ";";
    return value;
};
let storedOr = target[key("slot")] ||= rhs("slot", 10);
let keptOr = target[key("keep")] ||= rhs("keep", "bad");
let storedNullish = target[key("empty")] ??= rhs("empty", 11);
let storedAnd = target[key("yes")] &&= rhs("yes", 12);
let keptAnd = target[key("no")] &&= rhs("no", 13);
print(storedOr, keptOr, storedNullish, storedAnd, keptAnd);
print(target.slot, target.keep, target.empty, target.yes, target.no);

let values = [0, null, true, false];
let indexOrder = "";
let index = function(value) {
    indexOrder += "i" + value + ";";
    return value;
};
let valueRhs = function(label, value) {
    indexOrder += "v" + label + ";";
    return value;
};
let arrayOr = values[index(0)] ||= valueRhs("zero", 21);
let arrayNullish = values[index(1)] ??= valueRhs("null", 22);
let arrayAnd = values[index(2)] &&= valueRhs("true", 23);
let arraySkip = values[index(3)] &&= valueRhs("false", 24);
print(indexOrder, arrayOr, arrayNullish, arrayAnd, arraySkip);
print(values[0], values[1], values[2], values[3]);

let parenthesizedHead = (false || null) ?? 31;
let parenthesizedTail = null ?? (false || 32);
print(parenthesizedHead, parenthesizedTail);

order === "abceghkslot;rslot;kkeep;kempty;rempty;kyes;ryes;kno;" &&
    followsNull === 7 &&
    keepsZero === 0 &&
    keepsFalse === false &&
    followsUndefined === "fallback" &&
    andValue === 6 &&
    skippedAnd === 0 &&
    orValue === 5 &&
    skippedOr === 6 &&
    nullishValue === 4 &&
    undefinedValue === 3 &&
    truthy === 6 &&
    falsy === 5 &&
    empty === 4 &&
    missing === 3 &&
    storedOr === 10 &&
    keptOr === "value" &&
    storedNullish === 11 &&
    storedAnd === 12 &&
    keptAnd === false &&
    target.slot === 10 &&
    target.keep === "value" &&
    target.empty === 11 &&
    target.yes === 12 &&
    target.no === false &&
    indexOrder === "i0;vzero;i1;vnull;i2;vtrue;i3;" &&
    arrayOr === 21 &&
    arrayNullish === 22 &&
    arrayAnd === 23 &&
    arraySkip === false &&
    values[0] === 21 &&
    values[1] === 22 &&
    values[2] === 23 &&
    values[3] === false &&
    parenthesizedHead === 31 &&
    parenthesizedTail === 32
"#;

const LOGICAL_ASSIGNMENT_OUTPUT: [&str; 7] = [
    "abcegh 7 0 false fallback",
    "6 0 5 6 4 3",
    "10 value 11 12 false",
    "10 value 11 12 false",
    "i0;vzero;i1;vnull;i2;vtrue;i3; 21 22 23 false",
    "21 22 23 false",
    "31 32",
];

#[test]
fn supports_nullish_coalescing_and_logical_assignment() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(LOGICAL_ASSIGNMENT_SOURCE)?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(context.output(), &LOGICAL_ASSIGNMENT_OUTPUT)
}

#[test]
fn rejects_unparenthesized_nullish_logical_mixing() -> TestResult {
    ensure_error_contains(
        "let value = 1 || null ?? 2;",
        "cannot be mixed with '&&' or '||'",
    )?;
    ensure_error_contains(
        "let value = 1 && null ?? 2;",
        "cannot be mixed with '&&' or '||'",
    )?;
    ensure_error_contains(
        "let value = 1 ?? null || 2;",
        "expected statement terminator after variable declaration",
    )
}

#[test]
fn parenthesized_logical_assignment_does_not_infer_anonymous_function_names() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let left;
        let right;
        (left) ||= function() {};
        (right) ??= class {};
        left.name === "" && right.name === "" ? 42 : 0
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
    let matches = actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied());
    if matches {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    error_contains(&error, expected)
}

fn error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
