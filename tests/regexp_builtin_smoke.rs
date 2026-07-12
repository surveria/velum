use rs_quickjs::{Error, Runtime, RuntimeLimits, Value};

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
        let astral = /😀/du.exec("x😀y");
        let astralGlobal = /😀/gu;
        let astralGlobalMatch = astralGlobal.exec("x😀y");
        let unknownScript = /\p{Script_Extensions=Unknown}/u;
        let unknownPrimaryScript = /\p{Script=Unknown}/u;
        let notUnknownPrimaryScript = /\P{sc=Zzzz}/u;
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
            astral.index === 1 &&
            astral.indices[0][0] === 1 &&
            astral.indices[0][1] === 3 &&
            astralGlobalMatch.index === 1 &&
            astralGlobal.lastIndex === 3 &&
            "😀a".search(/a/u) === 2 &&
            "x😀y".replace(/😀/u, "z") === "xzy" &&
            "😀".match(/(?:)/gu).length === 2 &&
            unknownScript.test("\u{0378}") &&
            unknownScript.test("\uE000") &&
            !unknownScript.test("A") &&
            unknownPrimaryScript.test("\u{0378}") &&
            !unknownPrimaryScript.test("A") &&
            notUnknownPrimaryScript.test("A") &&
            !notUnknownPrimaryScript.test("\u{0378}") &&
            syntaxErrors === 2 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_observable_string_and_regexp_split_protocols() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let order = [];
        let receiver = {
            toString: function() {
                order.push("receiver");
                return "a,b";
            }
        };
        let separator = {};
        Object.defineProperty(separator, Symbol.split, {
            get: function() {
                order.push("method");
                return function(value, limit) {
                    order.push(value === receiver ? "call" : "wrong");
                    return [limit, this === separator];
                };
            }
        });
        let dispatched = String.prototype.split.call(receiver, separator, 7);

        function Splitter(pattern, flags) {
            this.pattern = pattern;
            this.flags = flags;
            this.lastIndex = 0;
            this.exec = function(input) {
                if (this.lastIndex !== 1) return null;
                this.lastIndex = 2;
                return { 0: ",", 1: "capture", length: 2 };
            };
        }
        let regexp = /,/;
        let species = {};
        Object.defineProperty(species, Symbol.species, { value: Splitter });
        regexp.constructor = species;
        let split = regexp[Symbol.split]("a,b", 3);

        let surrogateParts = "\uD83D\uDE00".split("");
        order.join("|") === "method|call" &&
            dispatched[0] === 7 &&
            dispatched[1] === true &&
            split.length === 3 &&
            split[0] === "a" &&
            split[1] === "capture" &&
            split[2] === "b" &&
            surrogateParts.length === 2 &&
            surrogateParts[0].charCodeAt(0) === 0xD83D &&
            surrogateParts[1].charCodeAt(0) === 0xDE00 &&
            "1001".split(1, 1)[0] === "" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_observable_string_and_regexp_replace_protocols() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let order = [];
        let receiver = { toString: function() { order.push("receiver"); return "abc"; } };
        let search = {};
        Object.defineProperty(search, Symbol.replace, {
            get: function() {
                order.push("method");
                return function(value, replacement) {
                    order.push(value === receiver && this === search ? "call" : "wrong");
                    return replacement;
                };
            }
        });
        let dispatched = String.prototype.replace.call(receiver, search, "ok");

        let regexp = /x/;
        regexp.global = true;
        let calls = 0;
        regexp.exec = function() {
            calls += 1;
            return calls === 1 ? { 0: "x", 1: 7, length: 2, index: 1, groups: { n: "g" } } : null;
        };
        let callbackArgs = [];
        let replaced = regexp[Symbol.replace]("axb", function() {
            callbackArgs = Array.from(arguments);
            return "R";
        });

        let unicode = /^|\udf06/g;
        Object.defineProperty(unicode, "unicode", { writable: true });
        unicode.unicode = false;
        let splitPair = unicode[Symbol.replace]("\ud834\udf06", "X");
        unicode.unicode = true;
        let wholePair = unicode[Symbol.replace]("\ud834\udf06", "X");

        order.join("|") === "method|call" &&
            dispatched === "ok" &&
            replaced === "aRb" &&
            callbackArgs.length === 5 &&
            callbackArgs[0] === "x" && callbackArgs[1] === "7" &&
            callbackArgs[2] === 1 && callbackArgs[3] === "axb" &&
            callbackArgs[4].n === "g" &&
            /b(c)(z)?(.)/[Symbol.replace]("abcde", "[$01$02$03$00]") === "a[cd$00]e" &&
            splitPair.length === 3 && splitPair.charCodeAt(1) === 0xD834 &&
            wholePair.length === 3 && wholePair.charCodeAt(1) === 0xD834 &&
            wholePair.charCodeAt(2) === 0xDF06 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_observable_regexp_match_search_and_match_all_protocols() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let matchCalls = 0;
        let matcher = {
            flags: "g",
            lastIndex: 0,
            exec: function() {
                matchCalls += 1;
                return matchCalls === 1 ? { 0: "", length: 1 } : null;
            }
        };
        let matches = RegExp.prototype[Symbol.match].call(matcher, "ab");

        let searchReceiver = {
            lastIndex: 7,
            exec: function() {
                this.lastIndex = 3;
                return { index: "marker" };
            }
        };
        let search = RegExp.prototype[Symbol.search].call(searchReceiver, "ab");

        let iteratorCalls = [];
        function IteratorMatcher(pattern, flags) {
            iteratorCalls.push("construct:" + flags);
            this.lastIndex = 0;
            this.execCount = 0;
            this.exec = function(input) {
                iteratorCalls.push("exec:" + input);
                this.execCount += 1;
                return this.execCount === 1 ? { 0: "a", index: 0, length: 1 } : null;
            };
        }
        let source = {
            flags: "g",
            lastIndex: 0,
            constructor: { [Symbol.species]: IteratorMatcher }
        };
        let iterator = RegExp.prototype[Symbol.matchAll].call(source, "ab");
        let lazy = iteratorCalls.join("|") === "construct:g";
        let first = iterator.next();
        let second = iterator.next();

        matches.length === 1 && matches[0] === "" && matchCalls === 2 &&
            matcher.lastIndex === 1 &&
            search === "marker" && searchReceiver.lastIndex === 7 &&
            lazy && first.done === false && first.value[0] === "a" &&
            second.done === true &&
            iteratorCalls.join("|") === "construct:g|exec:ab|exec:ab" &&
            Object.prototype.toString.call(iterator) === "[object RegExp String Iterator]" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_oversized_regexp_substitution_before_materialization() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_string_len: 256,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let source = r#"
        let capture = "a".repeat(64);
        let replacement = "$1".repeat(8);
        capture.replace(/(.+)/g, replacement)
    "#;
    if context.eval(source).is_ok() {
        return Err("expected RegExp replacement string limit to fail".into());
    }
    Ok(())
}

#[test]
fn rejects_invalid_regexp_literals_during_parsing() -> TestResult {
    for source in ["/(/", "/a/gg", "/a/z"] {
        let runtime = Runtime::new();
        let mut context = runtime.context();
        match context.eval(source) {
            Err(Error::Lex { .. } | Error::Parse { .. }) => {}
            Err(error) => {
                return Err(format!("expected parse-phase RegExp error, got {error}").into());
            }
            Ok(value) => {
                return Err(format!("expected invalid RegExp literal, got {value:?}").into());
            }
        }
    }
    Ok(())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
