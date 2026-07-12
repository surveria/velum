use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let actual = eval(source)?;
    if actual == Value::String(expected.to_owned()) {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

#[test]
fn string_receivers_expose_boxed_indices_to_array_methods() -> TestResult {
    ensure_string(
        r#"
        const mapped = Array.prototype.map.call("abc", function (value, index, receiver) {
            return value + index + (typeof receiver === "object" ? "o" : "p");
        });
        const filtered = Array.prototype.filter.call("abcd", function (value) {
            return value === "b" || value === "d";
        });
        "" + mapped.join(",")
            + ":" + filtered.join("")
            + ":" + Array.prototype.indexOf.call("abc", "b")
            + ":" + Array.prototype.slice.call("abc", 1).join("")
            + ":" + Array.prototype.toReversed.call("abc").join("")
        "#,
        "a0o,b1o,c2o:bd:1:bc:cba",
    )
}

#[test]
fn empty_primitive_wrappers_follow_generic_array_semantics() -> TestResult {
    ensure_string(
        r#"
        let calls = 0;
        const every = Array.prototype.every.call(true, function () {
            calls += 1;
            return false;
        });
        const mapped = Array.prototype.map.call(42, function () {
            calls += 1;
            return 1;
        });
        const symbol = Symbol("receiver");
        "" + every + ":" + mapped.length + ":" + calls
            + ":" + (Array.prototype.pop.call(symbol) === undefined)
            + ":" + Array.prototype.push.call(false, "value")
        "#,
        "true:0:0:true:1",
    )
}

#[test]
fn callable_receivers_remain_the_callback_object() -> TestResult {
    ensure_string(
        r#"
        function receiver(left, right) {}
        receiver[0] = "a";
        receiver[1] = "b";
        let sameReceiver = true;
        const result = Array.prototype.map.call(receiver, function (value, index, object) {
            sameReceiver = sameReceiver && object === receiver;
            return value + index;
        });
        "" + result.join("") + ":" + sameReceiver
        "#,
        "a0b1:true",
    )
}

#[test]
fn nullish_receivers_still_throw_before_callbacks() -> TestResult {
    ensure_string(
        r#"
        function kind(callback) {
            try {
                callback();
                return "none";
            } catch (error) {
                return error instanceof TypeError ? "TypeError" : "other";
            }
        }
        let calls = 0;
        "" + kind(function () {
            Array.prototype.map.call(null, function () { calls += 1; });
        }) + ":" + kind(function () {
            Array.prototype.indexOf.call(undefined, 1);
        }) + ":" + calls
        "#,
        "TypeError:TypeError:0",
    )
}
