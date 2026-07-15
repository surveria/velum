use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn tracks_legacy_regexp_match_state() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const matched = /b(c)(d)?(e)?/.exec("abcxef");
        const successful = matched[0] === "bc" &&
            RegExp.input === "abcxef" && RegExp.$_ === "abcxef" &&
            RegExp.lastMatch === "bc" && RegExp["$&"] === "bc" &&
            RegExp.lastParen === "c" && RegExp["$+"] === "c" &&
            RegExp.leftContext === "a" && RegExp["$`"] === "a" &&
            RegExp.rightContext === "xef" && RegExp["$'"] === "xef" &&
            RegExp.$1 === "c" && RegExp.$2 === "" && RegExp.$9 === "";
        const failed = /missing/.test("abcxef");
        const preserved = !failed && RegExp.lastMatch === "bc";
        RegExp.input = { toString() { return "manual\uD800input"; } };
        const assigned = RegExp.input === "manual\uD800input" &&
            RegExp.$_ === "manual\uD800input" && RegExp.lastMatch === "bc";
        successful && preserved && assigned ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))?;
    context.storage_snapshot()?;
    context.collect_garbage()?;
    context.storage_snapshot()?;
    Ok(())
}

#[test]
fn keeps_legacy_regexp_state_per_realm() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(r#"/(root)/.exec("root input");"#)?;
    let other = context.create_realm()?;
    let initial = context.eval_in_realm(&other, "RegExp.lastMatch;")?;
    ensure_value(&initial, &Value::from(""))?;
    let foreign = context.eval_in_realm(
        &other,
        r#"/(other)/.exec("other input"); RegExp.lastMatch;"#,
    )?;
    ensure_value(&foreign, &Value::from("other"))?;
    let root = context.eval("RegExp.lastMatch;")?;
    ensure_value(&root, &Value::from("root"))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
