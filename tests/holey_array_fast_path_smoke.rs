use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const HOLEY_ARRAY_FAST_PATH_SCRIPT: &str = r#"
var sparse = Array(4);
sparse[1] = "one";
sparse[3] = undefined;

var noProtoJoin = sparse.join("|") === "|one||";
var noProtoIncludes = sparse.includes(undefined) === true;
var noProtoIndex = sparse.indexOf(undefined) === 3;
var noProtoLast = sparse.lastIndexOf(undefined) === 3;

var noProtoSlice = sparse.slice(0, 4);
var noProtoSliceOk =
    noProtoSlice.length === 4 &&
    !("0" in noProtoSlice) &&
    noProtoSlice[1] === "one" &&
    !("2" in noProtoSlice) &&
    ("3" in noProtoSlice) &&
    noProtoSlice[3] === undefined;

var noProtoConcat = [].concat(sparse);
var noProtoConcatOk =
    noProtoConcat.length === 4 &&
    !("0" in noProtoConcat) &&
    noProtoConcat[1] === "one" &&
    !("2" in noProtoConcat) &&
    ("3" in noProtoConcat) &&
    noProtoConcat[3] === undefined;

Array.prototype[0] = "proto-zero";
Array.prototype[2] = "proto-two";

var inheritedJoin = sparse.join("|") === "proto-zero|one|proto-two|";
var inheritedIncludes = sparse.includes("proto-two") === true;
var inheritedIndex = sparse.indexOf("proto-zero") === 0;
var inheritedLast = sparse.lastIndexOf("proto-two") === 2;
var inheritedSlice = sparse.slice(0, 3);
var inheritedConcat = [].concat(sparse);

delete Array.prototype[0];
delete Array.prototype[2];

var inheritedSliceOk =
    inheritedSlice.length === 3 &&
    inheritedSlice[0] === "proto-zero" &&
    inheritedSlice[1] === "one" &&
    inheritedSlice[2] === "proto-two";
var inheritedConcatOk =
    inheritedConcat.length === 4 &&
    inheritedConcat[0] === "proto-zero" &&
    inheritedConcat[1] === "one" &&
    inheritedConcat[2] === "proto-two" &&
    inheritedConcat[3] === undefined;

noProtoJoin &&
    noProtoIncludes &&
    noProtoIndex &&
    noProtoLast &&
    noProtoSliceOk &&
    noProtoConcatOk &&
    inheritedJoin &&
    inheritedIncludes &&
    inheritedIndex &&
    inheritedLast &&
    inheritedSliceOk &&
    inheritedConcatOk ? 42 : 0
"#;

#[test]
fn holey_array_fast_paths_fall_back_for_indexed_prototypes() -> TestResult {
    let runtime = Runtime::new();
    let script = runtime.compile(HOLEY_ARRAY_FAST_PATH_SCRIPT)?;
    let mut context = runtime.context();

    let value = context.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
