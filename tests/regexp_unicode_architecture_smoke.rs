use rs_quickjs::{Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn dynamic_compilation_preserves_exact_utf16_source_units() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const high = String.fromCharCode(0xD83D);
        const low = String.fromCharCode(0xDC38);
        const literal = eval("/" + high + "\\uDC38?/u");
        const inClass = eval("/[" + high + "\\uDC38]/");
        const stringValue = eval("'" + high + "'");
        const templateValue = eval("`" + low + "`");

        literal.exec(high)[0] === high &&
            literal.exec(high + low) === null &&
            inClass.exec(high + low)[0] === high &&
            literal.source.charCodeAt(0) === 0xD83D &&
            stringValue.charCodeAt(0) === 0xD83D &&
            templateValue.charCodeAt(0) === 0xDC38 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn regexp_observable_steps_reload_current_internal_state() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let prototypeRead = false;
        const flags = {
            toString() {
                if (!prototypeRead) throw new Error("prototype lookup was late");
                return "g";
            }
        };
        const original = /a/;
        const newTarget = Object.defineProperty(function() {}.bind(null), "prototype", {
            get() {
                prototypeRead = true;
                original.compile("b");
                return RegExp.prototype;
            }
        });
        const constructed = Reflect.construct(RegExp, [original, flags], newTarget);

        const recompiled = /old/g;
        recompiled.lastIndex = {
            valueOf() {
                recompiled.compile("new", "");
                return 0;
            }
        };
        const current = recompiled.exec("new");

        constructed.flags === "g" &&
            constructed.source === "a" && original.source === "b" &&
            Object.getPrototypeOf(constructed) === RegExp.prototype &&
            current[0] === "new" && recompiled.source === "new" ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn regexp_unicode_and_source_invariants_share_one_engine_path() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const legacyCase = !/(\u017F)/i.test("s") && !/(\u1E9E)/i.test("\xDF");
        const unicodeCase = /(\u017F)/iu.test("s") && /(\u1E9E)/iu.test("\xDF");
        const wordComplement = /[^\W]/iu.test("\u017F") && /[^\W]/iu.test("\u212A");
        const splitBackref = /foo(.+)bar\1/u.exec("foo\uD834bar\uD834\uDC00") === null;
        const source = RegExp("[/]").source === "[/]" && RegExp("/").source === "\\/";
        const groupName = /(?<\u{1d4d3}>x)/.exec("x").groups.𝓓 === "x" &&
            /(?<\ud835\udcd3>x)/.exec("x").groups.𝓓 === "x";
        legacyCase && unicodeCase && wordComplement && splitBackref && source && groupName ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn braced_unicode_escape_accepts_long_leading_zero_runs_boundedly() -> TestResult {
    const LIMIT: usize = 262_144;
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_source_len: LIMIT,
        max_string_len: LIMIT,
        max_runtime_steps: 1_000_000,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const zeros = "0".repeat(100000);
        const unicode = eval(`/\\u{${zeros}41}/u`).test("A");
        const legacy = /\u{41}/.exec("u".repeat(41))[0].length === 41;
        unicode && legacy ? 42 : 0
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
