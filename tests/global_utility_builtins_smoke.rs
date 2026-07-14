use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn global_numeric_utility_builtins_follow_basic_ecmascript_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let keys = "";
        for (let key in Number) {
            keys = keys + key + ";";
        }

        let parseIntInvalid = parseInt("2", 2);
        let parseFloatInvalid = parseFloat(".");
        let globalNaN = isNaN("not-a-number");
        let globalFinite = isFinite("42");
        let numberNaNNoCoerce = Number.isNaN("NaN");
        let numberFiniteNoCoerce = Number.isFinite("42");

        print(
            parseInt.name,
            parseInt.length,
            parseFloat.name,
            parseFloat.length,
            Number.isNaN.name,
            Number.isFinite.name
        );
        print(parseInt("  -0xF"), parseInt("11", 2), parseInt("12px", 10), parseInt("08"));
        print(parseFloat("  -1.25e2px"), parseFloat(".5x"), parseFloat("1.e2px"), parseFloat("Infinity!"));
        print("keys:" + keys);

        typeof parseInt === "function" &&
            typeof parseFloat === "function" &&
            typeof isNaN === "function" &&
            typeof isFinite === "function" &&
            parseInt.name === "parseInt" &&
            parseInt.length === 2 &&
            parseFloat.name === "parseFloat" &&
            parseFloat.length === 1 &&
            Number.parseInt === parseInt &&
            Number.parseFloat === parseFloat &&
            Number.isNaN.name === "isNaN" &&
            Number.isNaN.length === 1 &&
            Number.isFinite.name === "isFinite" &&
            Number.isFinite.length === 1 &&
            parseInt("  -0xF") === -15 &&
            parseInt("11", 2) === 3 &&
            parseInt("12px", 10) === 12 &&
            parseInt("08") === 8 &&
            parseInt("\uFEFF9") === 9 &&
            Number.parseInt("\uFEFF9") === 9 &&
            parseIntInvalid !== parseIntInvalid &&
            parseFloat("  -1.25e2px") === -125 &&
            parseFloat(".5x") === 0.5 &&
            parseFloat("1.e2px") === 100 &&
            parseFloat("\uFEFF1.5") === 1.5 &&
            parseFloat("Infinity!") === Infinity &&
            parseFloatInvalid !== parseFloatInvalid &&
            globalNaN === true &&
            globalFinite === true &&
            Number.isNaN(NaN) === true &&
            numberNaNNoCoerce === false &&
            Number.isFinite(42) === true &&
            numberFiniteNoCoerce === false &&
            keys === "" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "parseInt 2 parseFloat 1 isNaN isFinite".to_owned(),
            "-15 3 12 8".to_owned(),
            "-125 0.5 100 Infinity".to_owned(),
            "keys:".to_owned(),
        ],
    )
}

#[test]
fn uri_utility_builtins_encode_decode_and_report_uri_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let reserved = ";/?:@&=+$,#";
        let encodedUri = encodeURI("front camera?x=1&name=камера");
        let encodedComponent = encodeURIComponent("front camera?x=1&name=камера");
        let decodedUri = decodeURI("front%20camera%3Fx=1%26name=%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0");
        let decodedComponent = decodeURIComponent("front%20camera%3Fx%3D1%26name%3D%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0");
        let invalidPercent = false;
        let invalidUtf8 = false;
        let invalidSurrogates = 0;
        try {
            decodeURIComponent("%");
        } catch (error) {
            invalidPercent = error.name === "URIError";
        }
        try {
            decodeURIComponent("%E0%A4%A");
        } catch (error) {
            invalidUtf8 = error.name === "URIError";
        }
        try {
            encodeURI(String.fromCharCode(0xD800));
        } catch (error) {
            invalidSurrogates += error instanceof URIError ? 1 : 0;
        }
        try {
            encodeURIComponent(String.fromCharCode(0xDC00));
        } catch (error) {
            invalidSurrogates += error instanceof URIError ? 1 : 0;
        }

        print(encodedUri);
        print(encodedComponent);
        print(decodedUri);
        print(decodedComponent);

        encodeURI(reserved) === reserved &&
            encodeURIComponent(reserved) === "%3B%2F%3F%3A%40%26%3D%2B%24%2C%23" &&
            encodedUri === "front%20camera?x=1&name=%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0" &&
            encodedComponent === "front%20camera%3Fx%3D1%26name%3D%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0" &&
            decodedUri === "front camera%3Fx=1%26name=камера" &&
            decodedComponent === "front camera?x=1&name=камера" &&
            invalidPercent &&
            invalidUtf8 &&
            invalidSurrogates === 2 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "front%20camera?x=1&name=%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0".to_owned(),
            "front%20camera%3Fx%3D1%26name%3D%D0%BA%D0%B0%D0%BC%D0%B5%D1%80%D0%B0".to_owned(),
            "front camera%3Fx=1%26name=камера".to_owned(),
            "front camera?x=1&name=камера".to_owned(),
        ],
    )
}

#[test]
fn global_utility_calls_compile_to_guarded_direct_targets() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        let total = 0;
        total += parseInt("20", 10);
        total += parseFloat("1.5");
        total += isFinite("5") ? 2 : 0;
        total += isNaN("x") ? 3 : 0;
        total += Number.isFinite(7) ? 4 : 0;
        total += Number.isNaN(NaN) ? 5 : 0;
        total += encodeURIComponent("a b") === "a%20b" ? 6 : 0;
        total += decodeURIComponent("c%20d") === "c d" ? 0.5 : 0;
        total === 42 ? 42 : 0
        "#,
    )?;

    ensure_min_usize(script.usage().bytecode_direct_native_call_count(), 8)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn global_utility_direct_targets_preserve_shadowing_and_mutation() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let shadowed = vm.eval(
        r#"
        {
            let parseInt = function(value) {
                return "shadow:" + value;
            };
            parseInt("7") === "shadow:7" ? 42 : 0
        }
        "#,
    )?;
    ensure_value(&shadowed, &Value::Number(42.0))?;

    let mutated = vm.eval(
        r#"
        Number.isFinite = function(value) {
            return value === "patched";
        };
        Number.isFinite("patched") ? 42 : 0
        "#,
    )?;
    ensure_value(&mutated, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_min_usize(actual: usize, expected_minimum: usize) -> TestResult {
    if actual >= expected_minimum {
        return Ok(());
    }

    Err(format!("expected at least {expected_minimum}, got {actual}").into())
}
