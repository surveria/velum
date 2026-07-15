use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn to_string_reports_builtin_class_tags() -> TestResult {
    eval_is_42(
        r#"
        var op = Object.prototype;
        op.toString.call([]) === "[object Array]" &&
            op.toString.call(null) === "[object Null]" &&
            op.toString.call(undefined) === "[object Undefined]" &&
            op.toString.call(42) === "[object Number]" &&
            op.toString.call("s") === "[object String]" &&
            op.toString.call(true) === "[object Boolean]" &&
            op.toString.call(function () {}) === "[object Function]" &&
            op.toString.call(new Date()) === "[object Date]" &&
            op.toString.call(/x/) === "[object RegExp]" &&
            op.toString.call(new Error("e")) === "[object Error]" &&
            op.toString.call(Object.create(op)) === "[object Object]"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn to_string_honors_symbol_to_string_tag() -> TestResult {
    eval_is_42(
        r#"
        var op = Object.prototype;
        var tagged = Object.create(op);
        tagged[Symbol.toStringTag] = "Custom";
        var nonString = Object.create(op);
        nonString[Symbol.toStringTag] = 5;
        op.toString.call(tagged) === "[object Custom]" &&
            op.toString.call(nonString) === "[object Object]"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn value_of_returns_object_and_boxes_primitives() -> TestResult {
    eval_is_42(
        r#"
        var op = Object.prototype;
        op.valueOf.call(op) === op &&
            op.valueOf.call(5) instanceof Number &&
            op.valueOf.call("z") instanceof String &&
            op.valueOf.call(true) instanceof Boolean
            ? 42
            : 0
        "#,
    )
}

#[test]
fn is_prototype_of_walks_the_chain() -> TestResult {
    eval_is_42(
        r"
        var op = Object.prototype;
        var proto = Object.create(op);
        var child = Object.create(proto);
        var grandchild = Object.create(child);
        proto.isPrototypeOf(child) === true &&
            proto.isPrototypeOf(grandchild) === true &&
            child.isPrototypeOf(proto) === false &&
            proto.isPrototypeOf(proto) === false &&
            op.isPrototypeOf.call(op, 42) === false &&
            op.isPrototypeOf.call(op, null) === false
            ? 42
            : 0
        ",
    )
}

#[test]
fn from_entries_builds_objects_from_iterables() -> TestResult {
    eval_is_42(
        r#"
        var fe = Object.fromEntries([["a", 1], ["b", 2], ["dup", 3], ["dup", 4]]);
        var fromMap = Object.fromEntries(new Map([["x", 10], ["y", 20]]));
        fe.a === 1 &&
            fe.b === 2 &&
            fe.dup === 4 &&
            Object.keys(fe).join(",") === "a,b,dup" &&
            fromMap.x === 10 &&
            fromMap.y === 20
            ? 42
            : 0
        "#,
    )
}

#[test]
fn methods_reject_null_or_undefined_receivers() -> TestResult {
    eval_is_42(
        r"
        var op = Object.prototype;
        var count = 0;
        try { op.valueOf.call(undefined); } catch (e) { if (e instanceof TypeError) count += 1; }
        try { op.valueOf.call(null); } catch (e) { if (e instanceof TypeError) count += 1; }
        try { Object.fromEntries(undefined); } catch (e) { if (e instanceof TypeError) count += 1; }
        count === 3 ? 42 : 0
        ",
    )
}

#[test]
fn exposes_method_metadata() -> TestResult {
    eval_is_42(
        r#"
        var op = Object.prototype;
        op.toString.length === 0 &&
            op.valueOf.length === 0 &&
            op.toLocaleString.length === 0 &&
            op.isPrototypeOf.length === 1 &&
            Object.fromEntries.length === 1 &&
            op.toString.name === "toString" &&
            op.valueOf.name === "valueOf" &&
            op.isPrototypeOf.name === "isPrototypeOf" &&
            op.toLocaleString.name === "toLocaleString" &&
            Object.fromEntries.name === "fromEntries"
            ? 42
            : 0
        "#,
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
