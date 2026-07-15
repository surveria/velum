use velum::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const STRING_PROTOTYPE_METHODS_SOURCE: &str = r#"
let text = "Camera Stream";
let padded = " \tCamera Stream\n ";
let boxed = new String("Boxed");
let protoKeys = "";
for (let key in String.prototype) {
    protoKeys = protoKeys + key + ";";
}

let searchOk = text.charAt(0) === "C" &&
    text.charAt(-1) === "" &&
    text.charAt(99) === "" &&
    text.charCodeAt(1) === 97 &&
    text.charCodeAt(99) !== text.charCodeAt(99) &&
    text.includes("Stream") &&
    text.includes("mera", 2) &&
    !text.includes("Camera", 1) &&
    text.indexOf("a") === 1 &&
    text.indexOf("a", 2) === 5 &&
    text.indexOf("missing") === -1 &&
    text.lastIndexOf("a") === 11 &&
    text.lastIndexOf("a", 2) === 1 &&
    text.startsWith("Camera") &&
    text.startsWith("mera", 2) &&
    !text.startsWith("Camera", 1) &&
    text.endsWith("Stream") &&
    text.endsWith("Camera", 6) &&
    !text.endsWith("Camera", 5);

let sliceOk = text.slice(1, 6) === "amera" &&
    text.slice(-6, -1) === "Strea" &&
    text.slice(7) === "Stream" &&
    text.slice(8, 3) === "" &&
    text.substring(7, 13) === "Stream" &&
    text.substring(6, 0) === "Camera" &&
    text.substring(-4, 6) === "Camera";

let transformOk = "go".repeat(3) === "gogogo" &&
    "go".repeat(undefined) === "" &&
    "a".concat("b", 7, true) === "ab7true" &&
    padded.trim() === "Camera Stream" &&
    padded.trimStart() === "Camera Stream\n " &&
    padded.trimEnd() === " \tCamera Stream" &&
    "MiXeD".toLowerCase() === "mixed" &&
    "MiXeD".toUpperCase() === "MIXED";

let receiverOk = boxed.slice(1, 4) === "oxe" &&
    String.prototype.slice.call(12345, 1, 4) === "234" &&
    String.prototype.includes.call(true, "ru") &&
    String.prototype.concat.call("id-", 42) === "id-42";

let metadataOk = typeof String.prototype.slice === "function" &&
    String.prototype.slice.name === "slice" &&
    String.prototype.slice.length === 2 &&
    String.prototype.trim.length === 0 &&
    String.prototype.includes.length === 1 &&
    protoKeys === "";

print(text.slice(0, 6), text.substring(7), "go".repeat(2));
print(padded.trim(), padded.trimStart(), padded.trimEnd());
print(String.prototype.slice.name, String.prototype.slice.length);

searchOk && sliceOk && transformOk && receiverOk && metadataOk ? 42 : 0
"#;

const STRING_PROTOTYPE_CACHE_SOURCE: &str = r#"
let run = function(value) {
    return value.slice(0, 2) + ":" + value.includes("am") + ":" + value.indexOf("a");
};

let first = run("camera");
let second = run("camera");
String.prototype.slice = function() {
    return "patched";
};
let third = run("camera");

first === "ca:true:1" &&
    second === "ca:true:1" &&
    third === "patched:true:1" ? 42 : 0
"#;

#[test]
fn supports_string_prototype_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(STRING_PROTOTYPE_METHODS_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "Camera Stream gogo",
            "Camera Stream Camera Stream\n   \tCamera Stream",
            "slice 2",
        ],
    )
}

#[test]
fn case_conversion_uses_current_unicode_data_and_preserves_lone_surrogates() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        const loneHigh = String.fromCharCode(0xD800);
        const loneLow = String.fromCharCode(0xDC00);
        const garaySmallA = String.fromCodePoint(0x10D70);
        loneHigh.toUpperCase().charCodeAt(0) === 0xD800 &&
            loneLow.toLowerCase().charCodeAt(0) === 0xDC00 &&
            garaySmallA.toUpperCase().codePointAt(0) === 0x10D50 ? 42 : 0
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_nullish_string_prototype_receivers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let null_result = context.eval("String.prototype.trim.call(null)");
    ensure_eval_error(&null_result)?;
    let undefined_result = context.eval("String.prototype.indexOf.call(undefined, 'x')");
    ensure_eval_error(&undefined_result)
}

#[test]
fn caches_string_prototype_native_calls_without_stale_dispatch() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(STRING_PROTOTYPE_CACHE_SOURCE)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let usage = vm.resource_usage();
    ensure_at_least(
        usage.native_call_cache_misses,
        3,
        "native call cache misses",
    )?;
    ensure_at_least(usage.native_call_cache_hits, 2, "native call cache hits")?;
    ensure_at_least(
        usage.native_call_cache_slow_paths,
        1,
        "native call cache slow paths",
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    let actual: Vec<&str> = actual.iter().map(String::as_str).collect();
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_eval_error(result: &velum::Result<Value>) -> TestResult {
    if result.is_err() {
        return Ok(());
    }
    Err("expected evaluation to fail".into())
}

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }
    Err(format!("expected {label} >= {expected}, got {actual}").into())
}
