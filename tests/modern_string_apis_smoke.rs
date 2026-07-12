use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn replace_all_supports_plain_substitutions_and_callbacks() -> TestResult {
    expect_true(
        r#"
        var calls = [];
        var plain = "aba".replaceAll("a", "[$&][$`][$']");
        var callback = "ab-ab".replaceAll("ab", function(match, position, source) {
            calls.push(match + ":" + position + ":" + source);
            return "x";
        });
        plain === "[a][][ba]b[a][ab][]" && callback === "x-x" &&
            calls.join("|") === "ab:0:ab-ab|ab:3:ab-ab"
        "#,
    )
}

#[test]
fn replace_all_dispatches_regexp_replacement_with_captures() -> TestResult {
    expect_true(
        r#"
        var calls = [];
        var callback = "aba".replaceAll(/(a)/g, function(match, capture, position, source) {
            calls.push(match + capture + position + source);
            return capture.toUpperCase();
        });
        var template = "aba".replaceAll(/(a)/g, "<$1>");
        callback === "AbA" && template === "<a>b<a>" && calls.length === 2
        "#,
    )
}

#[test]
fn well_formed_helpers_preserve_pairs_and_replace_lone_surrogates() -> TestResult {
    expect_true(
        r"
        var high = String.fromCharCode(0xD800);
        var low = String.fromCharCode(0xDC00);
        var pair = high + low;
        !high.isWellFormed() && !low.isWellFormed() && pair.isWellFormed() &&
            high.toWellFormed().charCodeAt(0) === 0xFFFD &&
            low.toWellFormed().charCodeAt(0) === 0xFFFD &&
            pair.toWellFormed() === pair
        ",
    )
}

#[test]
fn string_iterator_yields_code_points_and_validates_receivers() -> TestResult {
    expect_true(
        r#"
        var iterator = "A😀B"[Symbol.iterator]();
        var first = iterator.next();
        var second = iterator.next();
        var third = iterator.next();
        var done = iterator.next();
        var rejected = false;
        try { Object.create(iterator).next(); } catch (error) {
            rejected = error.constructor === TypeError;
        }
        first.value === "A" && second.value === "😀" && third.value === "B" &&
            done.done && rejected && iterator[Symbol.iterator]() === iterator
        "#,
    )
}

#[test]
fn string_iterator_exposes_standard_metadata() -> TestResult {
    expect_true(
        r#"
        var method = String.prototype[Symbol.iterator];
        var iterator = method.call("x");
        var prototype = Object.getPrototypeOf(iterator);
        method.name === "[Symbol.iterator]" && method.length === 0 &&
            prototype[Symbol.toStringTag] === "String Iterator" &&
            prototype.next.name === "next" && prototype.next.length === 0
        "#,
    )
}

fn expect_true(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}
