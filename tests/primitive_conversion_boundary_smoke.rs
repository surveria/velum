use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn honors_symbol_to_primitive_hints() -> TestResult {
    eval_is_42(
        r#"
        let hints = "";
        let value = {};
        value[Symbol.toPrimitive] = function (hint) {
            hints = hints + hint;
            return 40;
        };
        let sum = value + 2;
        let number = Number(value);
        sum === 42 && number === 40 && hints === "defaultnumber" ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_ordinary_to_primitive_order() -> TestResult {
    eval_is_42(
        r#"
        let order = "";
        let value = {};
        value.valueOf = function () {
            order = order + "v";
            return {};
        };
        value.toString = function () {
            order = order + "s";
            return "42";
        };
        Number(value) === 42 && value == 42 && order === "vsvs" ? 42 : 0
        "#,
    )
}

#[test]
fn trims_all_ecmascript_whitespace_during_numeric_conversion() -> TestResult {
    eval_is_42(
        r#"
        Number("\uFEFF\u3000 42 \u2029") === 42 &&
            Number("\uFEFF") === 0 ? 42 : 0
        "#,
    )
}

#[test]
fn routes_numeric_consumers_through_to_number() -> TestResult {
    let cases = [
        ("unary plus", "+value === 40"),
        ("unary minus", "-value === -40"),
        ("addition", "value + 2 === 42"),
        ("relational comparison", "value < 42"),
        ("bitwise conversion", "(value | 2) === 42"),
        ("boxed addition", "boxed + 2 === 42"),
        ("Math conversion", "Math.abs(value) === 40"),
        ("array index conversion", "array.slice(index).length === 2"),
    ];
    for (name, expression) in cases {
        let source = format!(
            r"
            let value = {{}};
            value.valueOf = function () {{ return 40; }};
            let boxed = new Number(40);
            let array = [0, 1, 2, 3];
            let index = {{}};
            index.valueOf = function () {{ return 2; }};
            ({expression}) ? 42 : 0
            "
        );
        eval_is_42(&source)
            .map_err(|error| -> Box<dyn std::error::Error> { format!("{name}: {error}").into() })?;
    }
    Ok(())
}

#[test]
fn rejects_invalid_primitive_conversions() -> TestResult {
    eval_is_42(
        r#"
        let failures = 40;
        try {
            +Symbol("number");
        } catch (error) {
            failures = failures + 1;
        }
        let value = {};
        value.valueOf = function () { return {}; };
        value.toString = function () { return {}; };
        try {
            Number(value);
        } catch (error) {
            failures = failures + 1;
        }
        failures
        "#,
    )
}

#[test]
fn preserves_array_search_conversion_order() -> TestResult {
    eval_is_42(
        r"
        let calls = 0;
        let fromEmpty = {};
        fromEmpty.valueOf = function () {
            calls = calls + 1;
            return 0;
        };
        let skipped = [].includes(0, fromEmpty) === false && calls === 0;

        let values = [1, 2, 3];
        let fromMutable = {};
        fromMutable.valueOf = function () {
            values.length = 0;
            return 0;
        };
        let result = values.indexOf(9, fromMutable);
        skipped && result === -1 && values.length === 0 ? 42 : 0
        ",
    )
}

#[test]
fn compares_strings_by_utf16_code_units() -> TestResult {
    eval_is_42(
        r#"
        "\uD7FF" < "\u{10000}" &&
            "\uD800" < "\uDC00" &&
            "\u{10000}" < "\uFFFF" &&
            "\uDC00" > "\uD800" ? 42 : 0
        "#,
    )
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected value 42, got {value:?}").into())
}
