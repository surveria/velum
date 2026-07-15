use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn supports_no_substitution_template_literals() -> TestResult {
    let value = eval(
        r"
        let empty = ``;
        let text = `front`;
        let escaped = `\`\$\\`;
        let lines = `north
south`;
        empty + ':' + text + ':' + escaped + ':' + lines
        ",
    )?;

    ensure_value(&value, &Value::from(":front:`$\\:north\nsouth"))
}

#[test]
fn templates_and_escaped_strings_do_not_enable_strict_mode() -> TestResult {
    let value = eval(
        r#"
        function escaped(eval) {
            "use\x20strict";
            return eval;
        }
        function templated(eval) {
            `use strict`;
            return eval;
        }
        escaped("escaped") + ":" + templated("template")
        "#,
    )?;

    ensure_value(&value, &Value::from("escaped:template"))
}

#[test]
fn no_substitution_templates_stay_out_of_string_literal_grammar() -> TestResult {
    ensure_error_contains("({ `name`: 1 })", "expected object property name")
}

#[test]
fn substitutes_expressions_with_to_string_semantics() -> TestResult {
    let value = eval(
        r"
        let count = 5;
        `count=${count}, twice=${count * 2}, flag=${true}, none=${null}:${undefined}`
        ",
    )?;

    ensure_value(
        &value,
        &Value::String(
            "count=5, twice=10, flag=true, none=null:undefined"
                .to_owned()
                .into(),
        ),
    )
}

#[test]
fn substitutes_adjacent_and_empty_parts() -> TestResult {
    let value = eval(r#"`${1}${""}${2}${3}`"#)?;
    ensure_value(&value, &Value::from("123"))
}

#[test]
fn supports_nested_template_literals_and_braces() -> TestResult {
    let value = eval(
        r"
        let inner = 40 + 2;
        `outer ${`inner ${inner}`} object ${ {answer: inner} } end`
        ",
    )?;

    ensure_value(
        &value,
        &Value::String(
            "outer inner 42 object [object Object] end"
                .to_owned()
                .into(),
        ),
    )
}

#[test]
fn substitution_can_call_functions_and_conditionals() -> TestResult {
    let value = eval(
        r#"
        function label(name) {
            return "<" + name + ">";
        }
        `call ${label("x")} pick ${1 ? "yes" : "no"}`
        "#,
    )?;

    ensure_value(&value, &Value::from("call <x> pick yes"))
}

#[test]
fn template_line_terminators_and_escapes_stay_cooked() -> TestResult {
    let value = eval("`first\r\nsecond ${1} \\${literal} \\` done`")?;
    ensure_value(&value, &Value::from("first\nsecond 1 ${literal} ` done"))
}

#[test]
fn symbol_substitution_throws_type_error() -> TestResult {
    let value = eval(
        r#"
        let caught = "";
        try {
            `${Symbol("marker")}`;
        } catch (error) {
            caught = (error instanceof TypeError) + ":" + error.name;
        }
        caught
        "#,
    )?;

    ensure_value(&value, &Value::from("true:TypeError"))
}

#[test]
fn regexp_literal_is_allowed_inside_substitution() -> TestResult {
    let value = eval("`re ${ /ab+c/.source } end`")?;
    ensure_value(&value, &Value::from("re ab+c end"))
}

#[test]
fn tagged_templates_expose_frozen_cooked_and_raw_arrays() -> TestResult {
    let value = eval(
        r#"
        let observed;
        function tag(strings, value) {
            observed = strings;
            return strings[0] + value + strings[1];
        }
        const result = tag`line\n${42}tail`;
        const raw = Object.getOwnPropertyDescriptor(observed, "raw");
        result === "line\n42tail"
            && observed[0] === "line\n"
            && observed.raw[0] === "line\\n"
            && Object.isFrozen(observed)
            && Object.isFrozen(observed.raw)
            && raw.enumerable === false
            && raw.writable === false
            && raw.configurable === false;
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn tagged_templates_cache_each_site_and_preserve_call_context() -> TestResult {
    let value = eval(
        r#"
        const templates = [];
        let receiver;
        const holder = {
            tag: function(strings) {
                receiver = this;
                templates.push(strings);
                return function Constructor(value) { this.value = value; };
            }
        };
        function run() { return holder.tag`site`; }
        run();
        run();
        const instance = new holder.tag`constructor`("value");
        receiver === holder
            && templates[0] === templates[1]
            && templates[1] !== templates[2]
            && instance.value === "value";
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn tagged_template_tail_calls_reuse_the_function_activation() -> TestResult {
    let value = eval(
        r#"
        "use strict";
        function direct(_, remaining) {
            if (remaining === 0) {
                return 1;
            }
            return direct`${remaining - 1}`;
        }
        function getIndirect() {
            return indirect;
        }
        function indirect(_, remaining) {
            if (remaining === 0) {
                return 1;
            }
            return getIndirect()`${remaining - 1}`;
        }
        direct(null, 400) + indirect(null, 400);
        "#,
    )?;
    ensure_value(&value, &Value::Number(2.0))
}

#[test]
fn tagged_templates_preserve_invalid_escape_raw_text() -> TestResult {
    let value = eval(
        r#"
        let cooked;
        let raw;
        (strings => { cooked = strings[0]; raw = strings.raw[0]; })`\xg`;
        cooked === undefined && raw === "\\xg";
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))?;
    ensure_error_contains(
        r"`\xg`",
        "invalid escape sequence in untagged template literal",
    )
}

#[test]
fn rejects_empty_substitution() -> TestResult {
    ensure_error_contains("`hello ${}`", "expected expression")
}

#[test]
fn rejects_unterminated_template_literal() -> TestResult {
    ensure_error_contains("`unterminated", "unterminated template literal")
}

#[test]
fn rejects_unterminated_substitution() -> TestResult {
    ensure_error_contains(
        "`hello ${name",
        "unterminated template literal substitution",
    )
}

#[test]
fn rejects_unterminated_substitution_with_open_brace() -> TestResult {
    ensure_error_contains(
        "`hello ${ {a: 1 }",
        "unterminated template literal substitution",
    )
}

#[test]
fn template_continuations_terminate_an_empty_yield_operand() -> TestResult {
    let value = eval(
        r#"
        function* values() {
            return `1${ yield }3${ 4 }5`;
        }
        let iterator = values();
        let first = iterator.next();
        let second = iterator.next(2);
        !first.done && first.value === undefined &&
            second.done && second.value === "12345" ? 42 : 0
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

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let Err(error) = eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    error_contains(&error, expected)
}

fn error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
