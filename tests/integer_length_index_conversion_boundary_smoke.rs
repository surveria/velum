use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn routes_integer_consumers_through_shared_conversion() -> TestResult {
    eval_is_42(
        r#"
        let hints = "";
        let index = {};
        index[Symbol.toPrimitive] = function (hint) {
            hints = hints + hint;
            return 1.9;
        };
        let array = [40, 41, 42];
        let text = "abc";
        array.at(index) === 41 && text.at(index) === "b" &&
            text.charAt(-0.9) === "a" && text.charCodeAt(-0.9) === 97 &&
            text.slice(-0.01, 0) === "" && hints === "numbernumber"
            ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_full_to_length_range_for_array_like_objects() -> TestResult {
    eval_is_42(
        r"
        let large = { length: Infinity };
        large[9007199254740990] = 42;
        Array.prototype.at.call(large, 9007199254740990)
        ",
    )
}

#[test]
fn routes_array_like_and_regexp_lengths_through_to_length() -> TestResult {
    eval_is_42(
        r#"
        let hints = "";
        let length = {};
        length[Symbol.toPrimitive] = function (hint) {
            hints = hints + hint;
            return 2.9;
        };
        let list = { length: length, 0: 20, 1: 22 };
        function sum(a, b) { return a + b; }
        let applied = sum.apply(null, list);
        let reflected = Reflect.apply(sum, null, list);

        let regexp = /a/g;
        regexp.lastIndex = {
            valueOf: function () {
                hints = hints + "r";
                return 1.9;
            }
        };
        let matched = regexp.exec("ba");
        applied === 42 && reflected === 42 && matched.index === 1 &&
            hints === "numbernumberr" ? 42 : 0
        "#,
    )
}

#[test]
fn applies_to_index_to_array_buffer_lengths() -> TestResult {
    eval_is_42(
        r#"
        let hints = "";
        let length = {};
        length[Symbol.toPrimitive] = function (hint) {
            hints = hints + hint;
            return 3.9;
        };
        let buffer = new ArrayBuffer(length);
        buffer.byteLength === 3 && hints === "number" ? 42 : 0
        "#,
    )
}

#[test]
fn applies_array_like_length_to_uint8_array_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let hints = "";
        let length = {};
        length[Symbol.toPrimitive] = function (hint) {
            hints = hints + hint;
            return 4.9;
        };
        let array = new Uint8Array({ length: length });
        "#,
    )?;
    let array = context.eval("array")?;
    let origin = context.typed_array_debug_origin(&array)?;
    if origin != Some("engine-owned") {
        return Err(
            format!("expected engine-owned Uint8Array, got {array:?} with {origin:?}").into(),
        );
    }
    let value = context.eval(
        r#"
        array[3] = 42;
        array[3] === 42 && array[4] === undefined && hints === "number" ? 42 : 0
        "#,
    )?;
    ensure_is_42(&value)
}

#[test]
fn defaults_missing_to_index_to_zero() -> TestResult {
    eval_is_42(
        r"
        let buffer = new ArrayBuffer();
        let array = new Uint8Array();
        buffer.byteLength === 0 && array.length === 0 ? 42 : 0
        ",
    )
}

#[test]
fn rejects_out_of_range_to_index_values() -> TestResult {
    eval_is_42(
        r#"
        let score = 37;
        try { new ArrayBuffer(-1); } catch (error) {
            score = score + (error instanceof RangeError ? 1 : 0);
        }
        try { new ArrayBuffer(Infinity); } catch (error) {
            score = score + (error instanceof RangeError ? 1 : 0);
        }
        try { new Uint8Array(9007199254740992); } catch (error) {
            score = score + (error instanceof RangeError ? 1 : 0);
        }
        try { new ArrayBuffer("1e309"); } catch (error) {
            score = score + (error instanceof RangeError ? 1 : 0);
        }
        try { new Uint8Array(Symbol("length")); } catch (error) {
            score = score + (error instanceof TypeError ? 1 : 0);
        }
        score
        "#,
    )
}

#[test]
fn reports_unsupported_byte_buffer_lengths_as_range_errors() -> TestResult {
    eval_is_42(
        r"
        let score = 40;
        try { new ArrayBuffer(4294967296); } catch (error) {
            score = score + (error instanceof RangeError ? 1 : 0);
        }
        try { new Uint8Array(4294967296); } catch (error) {
            score = score + (error instanceof RangeError ? 1 : 0);
        }
        score
        ",
    )
}

#[test]
fn keeps_infinity_string_parsing_case_sensitive() -> TestResult {
    eval_is_42(
        r#"
        Number("+Infinity") === Infinity &&
            Number("INFINITY") !== Number("INFINITY") &&
            Number("infinity") !== Number("infinity") ? 42 : 0
        "#,
    )
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_is_42(&value)
}

fn ensure_is_42(value: &Value) -> TestResult {
    if *value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected value 42, got {value:?}").into())
}
