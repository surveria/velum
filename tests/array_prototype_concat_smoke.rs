use rs_quickjs::{Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const CONCAT_SOURCE: &str = r#"
let values = [1, 2];
let tail = [3, 4];
let object = { marker: 7 };
let result = values.concat(tail, 5, object);

let side = 0;
let marker = function() {
    side = 42;
    return [8, 9];
};
let sideResult = [7].concat(marker());

let sparse = Array(4);
sparse[1] = "one";
sparse[3] = "three";
let sparseResult = ["zero"].concat(sparse, "tail");

Array.prototype[0] = "proto-zero";
let inherited = Array(2);
inherited[1] = "own-one";
let inheritedResult = [].concat(inherited);
delete Array.prototype[0];

Array.prototype[2] = "proto-two";
let prefixFallback = Array(4);
prefixFallback[0] = "own-zero";
prefixFallback[1] = "own-one";
prefixFallback[3] = "own-three";
let prefixFallbackResult = [].concat(prefixFallback);
delete Array.prototype[2];

let plain = {};
plain[0] = "plain-zero";
plain.length = 1;
let plainResult = [1].concat(plain);

let prototypeKeys = "";
for (let key in Array.prototype) {
    prototypeKeys = prototypeKeys + key + ";";
}

print("concat", result.length, result[0], result[1], result[2], result[3], result[4], result[5] === object);
print("source", values.length, values.join("|"), tail.join("|"));
print("side", side, sideResult.join("|"));
print("sparse", sparseResult.length, sparseResult[0], "1" in sparseResult, sparseResult[1], sparseResult[2], "3" in sparseResult, sparseResult[3], sparseResult[4], sparseResult[5], sparseResult.join("|"));
print("inherited", inheritedResult.length, inheritedResult[0], "0" in inheritedResult, inheritedResult[1]);
print("prefix", prefixFallbackResult.length, prefixFallbackResult[0], prefixFallbackResult[1], prefixFallbackResult[2], "2" in prefixFallbackResult, prefixFallbackResult[3], prefixFallbackResult.join("|"));
print("plain", plainResult.length, plainResult[0], plainResult[1] === plain);
print("meta", typeof Array.prototype.concat, Array.prototype.concat.name, Array.prototype.concat.length);
print("keys:" + prototypeKeys);
print("in", "concat" in values);

result.length === 6 &&
    result[0] === 1 &&
    result[1] === 2 &&
    result[2] === 3 &&
    result[3] === 4 &&
    result[4] === 5 &&
    result[5] === object &&
    values.join("|") === "1|2" &&
    tail.join("|") === "3|4" &&
    side === 42 &&
    sideResult.join("|") === "7|8|9" &&
    sparseResult.length === 6 &&
    sparseResult[0] === "zero" &&
    !("1" in sparseResult) &&
    sparseResult[1] === undefined &&
    sparseResult[2] === "one" &&
    !("3" in sparseResult) &&
    sparseResult[3] === undefined &&
    sparseResult[4] === "three" &&
    sparseResult[5] === "tail" &&
    sparseResult.join("|") === "zero||one||three|tail" &&
    inheritedResult.length === 2 &&
    inheritedResult[0] === "proto-zero" &&
    ("0" in inheritedResult) &&
    inheritedResult[1] === "own-one" &&
    prefixFallbackResult.length === 4 &&
    prefixFallbackResult[0] === "own-zero" &&
    prefixFallbackResult[1] === "own-one" &&
    prefixFallbackResult[2] === "proto-two" &&
    ("2" in prefixFallbackResult) &&
    prefixFallbackResult[3] === "own-three" &&
    prefixFallbackResult.join("|") === "own-zero|own-one|proto-two|own-three" &&
    plainResult.length === 2 &&
    plainResult[0] === 1 &&
    plainResult[1] === plain &&
    typeof Array.prototype.concat === "function" &&
    Array.prototype.concat.name === "concat" &&
    Array.prototype.concat.length === 1 &&
    prototypeKeys === "" &&
    ("concat" in values) ? 42 : 0
"#;

const CONCAT_OUTPUT: &[&str] = &[
    "concat 6 1 2 3 4 5 true",
    "source 2 1|2 3|4",
    "side 42 7|8|9",
    "sparse 6 zero false undefined one false undefined three tail zero||one||three|tail",
    "inherited 2 proto-zero true own-one",
    "prefix 4 own-zero own-one proto-two true own-three own-zero|own-one|proto-two|own-three",
    "plain 2 1 true",
    "meta function concat 1",
    "keys:",
    "in true",
];

#[test]
fn supports_array_concat_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(CONCAT_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), CONCAT_OUTPUT)
}

#[test]
fn supports_array_concat_on_non_array_receiver() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { 0: "first", length: 1 };
        object.concat = Array.prototype.concat;
        let result = object.concat(["second"]);
        result.length === 2 && result[0] === object && result[1] === "second" ? 42 : 0;
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn honors_concat_spreadability_and_sparse_array_like_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        const sparse = { 1: "one", 3: "three", length: 4 };
        sparse[Symbol.isConcatSpreadable] = true;
        const nested = ["nested"];
        nested[Symbol.isConcatSpreadable] = false;
        const result = ["zero"].concat(sparse, nested, "tail");
        "" + result.length + ":" + result[0]
            + ":" + (1 in result) + ":" + result[2]
            + ":" + (3 in result) + ":" + result[4]
            + ":" + (result[5] === nested) + ":" + result[6]
        "#,
    )?;
    ensure_value(
        &value,
        &Value::from("7:zero:false:one:false:three:true:tail"),
    )
}

#[test]
fn treats_proxy_wrapped_arrays_as_spreadable() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        const reads = [];
        const proxy = new Proxy([1, 2], {
            get: function (target, property, receiver) {
                reads.push(property === Symbol.isConcatSpreadable ? "spreadable" : String(property));
                return Reflect.get(target, property, receiver);
            }
        });
        const result = [0].concat(proxy);
        const revocable = Proxy.revocable([], {});
        revocable.revoke();
        let revokedThrows = false;
        try {
            [].concat(revocable.proxy);
        } catch (error) {
            revokedThrows = error instanceof TypeError;
        }
        result.join(",") === "0,1,2" &&
            reads.join(",") === "spreadable,length,0,1" &&
            revokedThrows ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn uses_array_species_before_spreadability_lookup() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        const calls = [];
        const source = [];
        function Result(length) {
            calls.push("species:" + length);
            this.length = length;
            this.marker = "custom";
        }
        Object.defineProperty(source, "constructor", {
            configurable: true,
            get: function () {
                calls.push("constructor");
                return { [Symbol.species]: Result };
            }
        });
        Object.defineProperty(source, Symbol.isConcatSpreadable, {
            configurable: true,
            get: function () {
                calls.push("spreadable");
                return true;
            }
        });
        const result = source.concat("value");
        "" + result.marker + ":" + result.length + ":" + result[0]
            + ":" + calls.join(",")
        "#,
    )?;
    ensure_value(
        &value,
        &Value::String(
            "custom:1:value:constructor,species:0,spreadable"
                .to_owned()
                .into(),
        ),
    )
}

#[test]
fn propagates_spreadable_and_length_getter_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function kind(callback) {
            try {
                callback();
                return "none";
            } catch (error) {
                return error.name;
            }
        }
        const spreadableError = {};
        Object.defineProperty(spreadableError, Symbol.isConcatSpreadable, {
            get: function () { throw new RangeError("spreadable"); }
        });
        const lengthError = { [Symbol.isConcatSpreadable]: true };
        Object.defineProperty(lengthError, "length", {
            get: function () { throw new SyntaxError("length"); }
        });
        "" + kind(function () { [].concat(spreadableError); })
            + ":" + kind(function () { [].concat(lengthError); })
        "#,
    )?;
    ensure_value(&value, &Value::from("RangeError:SyntaxError"))
}

#[test]
fn counts_packed_concat_result_toward_property_limit() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_object_properties: 3,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();

    let Err(error) = context.eval("[1, 2].concat([3, 4])") else {
        return Err("expected packed Array.prototype.concat result limit to fail".into());
    };
    ensure_error_contains(&error, "object property count exceeded 3")
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
