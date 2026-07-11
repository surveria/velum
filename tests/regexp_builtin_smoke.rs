use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_minimal_regexp_literals_and_test_method() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let word = /\w/.test("abc") && !/\w/.test("-");
        let newline = /[\u000A\u000D\u2028\u2029]/.test("a\nb") &&
            !/[\u000A\u000D\u2028\u2029]/.test("abc");
        let whitespace = /[\u0009\u000B\u000C\u0020\u00A0\uFEFF]/.test("\t") &&
            !/[\u0009\u000B\u000C\u0020\u00A0\uFEFF]/.test("x");
        let spaceSeparator = /[ \xA0\u1680\u2000-\u200A\u202F\u205F\u3000]/.test(" ") &&
            !/[ \xA0\u1680\u2000-\u200A\u202F\u205F\u3000]/.test("x");
        let identifierStart = /(?:[A-Za-z\xAA\u00B5])/.test("A") &&
            /(?:[A-Za-z\xAA\u00B5])/.test("µ") &&
            !/(?:[A-Za-z\xAA\u00B5])/.test("0");
        let identifierContinue = /(?:[0-9A-Z_a-z\xAA\u00B5])/.test("0") &&
            /(?:[0-9A-Z_a-z\xAA\u00B5])/.test("_") &&
            !/(?:[0-9A-Z_a-z\xAA\u00B5])/.test("-");
        let literal = /camera/i.test("CAMERA-01") && !/camera/.test("CAMERA-01");
        let metadata = typeof RegExp === "function" &&
            RegExp.name === "RegExp" &&
            RegExp.length === 2 &&
            typeof RegExp.prototype.test === "function" &&
            RegExp.prototype.test.name === "test" &&
            RegExp.prototype.test.length === 1;
        let regexp = /\w/;
        let source = regexp.source === "\\w" && regexp.flags === "";

        word &&
            newline &&
            whitespace &&
            spaceSeparator &&
            identifierStart &&
            identifierContinue &&
            literal &&
            metadata &&
            source ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_regexp_constructor_and_preserves_slash_operator() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let constructor = RegExp("abc").test("zabcq") && !RegExp("abc").test("zzz");
        let quotient = 8 / 2;
        quotient /= 2;
        function returnedLiteral() {
            return /abc/.test("abc");
        }
        constructor && quotient === 2 && returnedLiteral() ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_ecmascript_patterns_captures_and_match_indices() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let match = /(?<prefix>a|b)(c+)(?=d)/d.exec("xxacccd");
        let lookbehind = /(?<=key=)(\w+)/.exec("key=value");
        let backreference = /^(a|b)\1$/.test("aa") && !/^(a|b)\1$/.test("ab");
        let syntaxErrors = 0;
        try {
            new RegExp("(");
        } catch (error) {
            if (error instanceof SyntaxError) syntaxErrors += 1;
        }
        try {
            new RegExp("a", "gg");
        } catch (error) {
            if (error instanceof SyntaxError) syntaxErrors += 1;
        }

        match[0] === "accc" &&
            match[1] === "a" &&
            match[2] === "ccc" &&
            match.length === 3 &&
            match.index === 2 &&
            match.input === "xxacccd" &&
            match.groups.prefix === "a" &&
            Object.getPrototypeOf(match.groups) === null &&
            match.indices[0][0] === 2 &&
            match.indices[0][1] === 6 &&
            match.indices[1][0] === 2 &&
            match.indices.groups.prefix[0] === 2 &&
            match.indices.groups.prefix[1] === 3 &&
            lookbehind[0] === "value" &&
            lookbehind[1] === "value" &&
            backreference &&
            syntaxErrors === 2 ? 42 : 0
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
