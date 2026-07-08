use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const STRING_STATIC_UNICODE_SOURCE: &str = r#"
let raw = String.raw({ raw: { 0: "a", 1: "b", 2: "c", length: 3 } }, 1, 2);
let boxed = new String("Boxed");

let staticOk = String.fromCharCode(67, 97, 109) === "Cam" &&
    String.fromCharCode(65.9, -1) === "A\uFFFF" &&
    String.fromCodePoint(0x2603).codePointAt(0) === 0x2603 &&
    raw === "a1b2c";

let prototypeOk = "camera".at(0) === "c" &&
    "camera".at(-1) === "a" &&
    "camera".at(99) === undefined &&
    "snow\u2603".codePointAt(4) === 0x2603 &&
    "cam".padStart(5, "0") === "00cam" &&
    "cam".padEnd(6, "ab") === "camaba" &&
    "cam".padStart(2, "0") === "cam" &&
    "cam".padEnd(6, "") === "cam" &&
    "\ttrim\n".trimLeft() === "trim\n" &&
    "\ttrim\n".trimRight() === "\ttrim" &&
    "MiXeD".toLocaleLowerCase() === "mixed" &&
    "MiXeD".toLocaleUpperCase() === "MIXED" &&
    String.prototype.toString.call(boxed) === "Boxed" &&
    String.prototype.valueOf.call(boxed) === "Boxed";

let metadataOk = String.fromCharCode.length === 1 &&
    String.fromCodePoint.length === 1 &&
    String.raw.length === 1 &&
    String.prototype.at.length === 1 &&
    String.prototype.codePointAt.length === 1 &&
    String.prototype.padStart.length === 1 &&
    String.prototype.padEnd.length === 1 &&
    String.prototype.toString.length === 0 &&
    String.prototype.valueOf.length === 0 &&
    String.prototype.trimLeft === String.prototype.trimStart &&
    String.prototype.trimRight === String.prototype.trimEnd;

print(String.fromCharCode(67, 97, 109));
print(String.fromCodePoint(0x2603).codePointAt(0));
print(raw);
print("camera".at(-1), "cam".padStart(5, "0"), "cam".padEnd(6, "ab"));

staticOk && prototypeOk && metadataOk ? 42 : 0
"#;

const DIRECT_CALL_TARGET_SOURCE: &str = r#"
let raw = { raw: { 0: "a", 1: "b", length: 2 } };
let output =
    String.fromCharCode(65, 66) +
    ":" +
    String.fromCodePoint(0x2603).codePointAt(0) +
    ":" +
    String.raw(raw, 1) +
    ":" +
    "camera".at(-1) +
    ":" +
    "cam".padStart(5, "0") +
    ":" +
    "cam".padEnd(5, "0") +
    ":" +
    "MiXeD".toLocaleLowerCase() +
    ":" +
    "MiXeD".toLocaleUpperCase();

output === "AB:9731:a1b:a:00cam:cam00:mixed:MIXED" ? 42 : 0
"#;

#[test]
fn supports_string_static_and_unicode_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(STRING_STATIC_UNICODE_SOURCE)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["Cam", "9731", "a1b2c", "a 00cam camaba"],
    )
}

#[test]
fn rejects_invalid_string_static_and_value_receivers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    ensure_eval_error_contains(
        context.eval("String.fromCodePoint(0x110000)"),
        "code point must be valid",
    )?;
    ensure_eval_error_contains(
        context.eval("String.prototype.toString.call(42)"),
        "requires a string or String object",
    )
}

#[test]
fn compiles_string_static_unicode_calls_as_direct_native_targets() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(DIRECT_CALL_TARGET_SOURCE)?;

    ensure_min_usize(script.usage().bytecode_direct_native_call_count(), 8)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
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

fn ensure_eval_error_contains(result: rs_quickjs::Result<Value>, expected: &str) -> TestResult {
    let Err(error) = result else {
        return Err(format!("expected evaluation to fail with '{expected}'").into());
    };
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error containing '{expected}', got {message}").into())
}

fn ensure_min_usize(actual: usize, expected_minimum: usize) -> TestResult {
    if actual >= expected_minimum {
        return Ok(());
    }
    Err(format!("expected at least {expected_minimum}, got {actual}").into())
}
