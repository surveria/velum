use velum::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_string_prototype_and_utf16_boundaries() -> TestResult {
    ensure_eval(
        r#"
        let originalToString = String.prototype.toString;
        delete String.prototype.toString;
        let prototypeOk = String.prototype == "" &&
            Object.prototype.isPrototypeOf(String.prototype) &&
            String.prototype.toString() === "[object String]";
        String.prototype.toString = originalToString;

        let throwing = { toString() { throw new Error("unused"); } };
        let rawOk = String.raw({ raw: ["a", "c", "e"] }, "b", "d", throwing) === "abcde";
        let boundaryOk = "abc".codePointAt(-1) === undefined &&
            "ABBABABAB".lastIndexOf({ toString() { return "AB"; } }, {
                valueOf() { return NaN; }
            }) === 7;
        let utf16Ok = "abc".padEnd(6, "\uD83D\uDCA9") === "abc\uD83D\uDCA9\uD83D" &&
            "abc".padStart(6, "\uD83D\uDCA9") === "\uD83D\uDCA9\uD83Dabc";

        prototypeOk && rawOk && boundaryOk && utf16Ok ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn promise_resolvers_settle_once_and_require_valid_receivers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let settled = "pending";
        let thenable = { then(resolve) { resolve(42); } };
        new Promise(function(resolve) {
            resolve(thenable);
            throw new Error("ignored");
        }).then(
            function(value) { settled = value; },
            function() { settled = "rejected"; }
        );
        "#,
    )?;
    let settled = context.eval("settled")?;
    let errors = context.eval(
        r"
        let errors = 0;
        try { Promise(function() {}); } catch (error) {
            if (error instanceof TypeError) errors += 1;
        }
        try { Promise.prototype.then.call({}); } catch (error) {
            if (error instanceof TypeError) errors += 1;
        }
        errors
        ",
    )?;
    if settled == Value::Number(42.0) && errors == Value::Number(2.0) {
        return Ok(());
    }
    Err(format!("expected settled 42 and two errors, got {settled:?} and {errors:?}").into())
}

#[test]
fn iterator_helpers_reject_reentrant_next_calls() -> TestResult {
    ensure_eval(
        r"
        let entries = 0;
        let iterator;
        iterator = Iterator.from([1]).map(function(value) {
            entries += 1;
            iterator.next();
            return value;
        });
        let caught = false;
        try { iterator.next(); } catch (error) { caught = error instanceof TypeError; }
        caught && entries === 1 ? 42 : 0
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn regexp_observes_last_index_and_duplicate_named_backreferences() -> TestResult {
    ensure_eval(
        r#"
        let gets = 0;
        let counter = { valueOf() { gets += 1; return 0; } };
        let regexp = /a/;
        regexp.lastIndex = counter;
        let lastIndexOk = regexp.exec("bab")[0] === "a" &&
            regexp.lastIndex === counter && gets === 1;

        let first = /(?:(?<x>a)|(?<x>b))\k<x>/.exec("aa");
        let second = /(?:(?<x>a)|(?<x>b))\k<x>/.exec("bb");
        let repeated = /(?:(?:(?<x>a)|(?<x>b))\k<x>){2}/.exec("aabb");
        let duplicatesOk = first[0] === "aa" && first[1] === "a" &&
            second[0] === "bb" && second[2] === "b" &&
            repeated[0] === "aabb" && repeated.groups.x === "b" &&
            /(?:(?:(?<x>a)|(?<x>b))\k<x>){2}/.test("abab") === false;
        let incompatibleFlags = false;
        try { new RegExp(".", "uv"); } catch (error) {
            incompatibleFlags = error instanceof SyntaxError;
        }
        lastIndexOk && duplicatesOk && incompatibleFlags ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if actual == *expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
