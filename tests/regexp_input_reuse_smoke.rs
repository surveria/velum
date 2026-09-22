use velum::{Engine, Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_exact_match_input_captures_and_legacy_roots_after_collection() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let input = context.eval(r#"globalThis.subject = "prefix\uD800x\uDC00tail"; subject"#)?;
    let matched_input = context.eval(
        r#"
        globalThis.matcher = /(?<unit>\uD800)x/dy;
        matcher.lastIndex = 6;
        globalThis.saved = matcher.exec(subject);
        saved.input
        "#,
    )?;
    let (Value::String(input), Value::String(matched_input)) = (&input, &matched_input) else {
        return Err("expected heap-owned subject and match input strings".into());
    };
    if input.id() != matched_input.id() || input.identity() != matched_input.identity() {
        return Err("RegExp match input changed its admitted string identity".into());
    }
    context.eval("globalThis.subject = null;")?;
    context.collect_garbage()?;
    context.storage_snapshot()?;
    let value = context.eval(
        r#"
        const input = saved.input;
        const matched = saved[0] === "\uD800x" && saved.index === 6 &&
            saved.groups.unit === "\uD800" && saved.indices[0][0] === 6 &&
            saved.indices[0][1] === 8 && saved.indices.groups.unit[0] === 6 &&
            saved.indices.groups.unit[1] === 7 && matcher.lastIndex === 8;
        const missed = matcher.exec(input) === null && matcher.lastIndex === 0;
        matched && missed && input.charCodeAt(6) === 0xD800 &&
            input.charCodeAt(8) === 0xDC00 && RegExp.input === input &&
            RegExp.lastMatch === "\uD800x" && RegExp.$1 === "\uD800" &&
            RegExp.leftContext === "prefix" && RegExp.rightContext === "\uDC00tail"
        "#,
    )?;
    ensure_true(&value)
}

#[test]
fn coerces_input_once_before_last_index_recompiles_the_matcher() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let log = "";
        const matcher = /old/g;
        const input = {
            toString() { log += "input;"; return "new\uD800"; }
        };
        matcher.lastIndex = {
            valueOf() {
                log += "index;";
                matcher.compile("new", "g");
                return 0;
            }
        };
        const matched = matcher.exec(input);
        log === "input;index;" && matched[0] === "new" &&
            matched.input === "new\uD800" && matched.input.charCodeAt(3) === 0xD800 &&
            matcher.lastIndex === 3 && RegExp.input === matched.input
        "#,
    )?;
    ensure_true(&value)
}

#[test]
fn honors_exec_getters_overrides_and_noncallable_fallback() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let log = "";
        const input = { toString() { log += "input;"; return "a\uD800"; } };
        const matcher = /a/g;
        Object.defineProperty(matcher, "exec", {
            configurable: true,
            get() {
                log += "exec;";
                return function(value) {
                    log += "call;";
                    return this === matcher && value === "a\uD800" ? {} : null;
                };
            }
        });
        const custom = matcher.test(input) && log === "input;exec;call;";
        Object.defineProperty(matcher, "exec", { value: 7 });
        const native = matcher.test(input) && matcher.lastIndex === 1 &&
            RegExp.input === "a\uD800" && !matcher.test(input) && matcher.lastIndex === 0;
        custom && native
        "#,
    )?;
    ensure_true(&value)
}

#[test]
fn preserves_throwing_last_index_writes_and_prior_legacy_state() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        /(prior)/.exec("prior subject");
        const matcher = /new/g;
        Object.defineProperty(matcher, "lastIndex", { writable: false });
        let failures = 0;
        for (const input of ["new", "missing"]) {
            try { matcher.exec(input); }
            catch (error) { if (error instanceof TypeError) failures += 1; }
        }
        failures === 2 && RegExp.lastMatch === "prior" && RegExp.input === "prior subject"
        "#,
    )?;
    ensure_true(&value)
}

#[test]
fn failed_coerced_matches_do_not_admit_unused_subject_strings() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    ensure_true(&vm.context().eval(
        "globalThis.matcher = /unmatched/; matcher.exec(123456789) === null",
    )?)?;
    let before = vm.resource_usage();
    ensure_true(&vm.context().eval("matcher.exec(987654321) === null")?)?;
    let after = vm.resource_usage();
    if before.string_count == after.string_count && before.string_bytes == after.string_bytes {
        return Ok(());
    }
    Err(format!("failed RegExp match changed string usage: {before:?} -> {after:?}").into())
}

#[test]
fn retains_reused_subjects_in_the_matching_realm_only() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(r#"/(?<part>root)/d.exec("root\uD800");"#)?;
    let other = context.create_realm()?;
    ensure_true(&context.eval_in_realm(
        &other,
        r#"
        const initiallyEmpty = RegExp.input === "";
        const matched = /(?<part>other)/d.exec("other\uDC00");
        initiallyEmpty && matched.input === "other\uDC00" && RegExp.$1 === "other"
        "#,
    )?)?;
    context.collect_garbage()?;
    context.storage_snapshot()?;
    ensure_true(&context.eval(r#"RegExp.input === "root\uD800" && RegExp.$1 === "root""#)?)?;
    ensure_true(&context.eval_in_realm(
        &other,
        r#"RegExp.input === "other\uDC00" && RegExp.$1 === "other""#,
    )?)
}

#[test]
fn regexp_input_cannot_import_a_foreign_vm_string() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let mut second = engine.create_vm();
    let foreign = first.context().eval(r#""foreign\uD800""#)?;
    second
        .context()
        .register_host_function("foreignInput", move |_call| Ok(foreign.clone()))?;
    let Err(error) = second.context().eval("/foreign/.exec(foreignInput())") else {
        return Err("RegExp execution accepted a foreign VM string".into());
    };
    if error.to_string().contains("value belongs to another VM") {
        return Ok(());
    }
    Err(format!("unexpected foreign RegExp input error: {error}").into())
}

fn ensure_true(value: &Value) -> TestResult {
    if value == &Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, received {value:?}").into())
}
