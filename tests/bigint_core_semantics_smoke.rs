use rs_quickjs::{OwnedValue, Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_arbitrary_precision_literals_and_numeric_operations() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let update = 7n;
        let old = update++;
        let prefix = ++update;
        let errors = 0;
        for (let source of [
            function () { return 1n + 1; },
            function () { return 1n / 0n; },
            function () { return 2n ** -1n; },
            function () { return 1n >>> 0n; },
            function () { return +1n; }
        ]) {
            try { source(); } catch (error) {
                if (error instanceof TypeError || error instanceof RangeError) errors++;
            }
        }

        typeof 1n === "bigint" &&
            90071992547409931234567890n + 10n === 90071992547409931234567900n &&
            0xffn === 255n && 0o77n === 63n && 0b101n === 5n &&
            -7n + 2n === -5n && 7n - 9n === -2n &&
            7n * 6n === 42n && 17n / 5n === 3n && 17n % 5n === 2n &&
            3n ** 4n === 81n && (~0n) === -1n &&
            (6n & 3n) === 2n && (4n | 1n) === 5n && (7n ^ 3n) === 4n &&
            (3n << 4n) === 48n && (-8n >> 2n) === -2n &&
            3n < 3.5 && 4n > 3.5 && 3n == 3 && 3n !== 3 &&
            old === 7n && update === 9n && prefix === 9n && errors === 5 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn exposes_constructor_boxing_and_prototype_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let boxed = Object(255n);
        let constructError = false;
        try { new BigInt(1); } catch (error) { constructError = error instanceof TypeError; }

        typeof BigInt === "function" && BigInt.name === "BigInt" && BigInt.length === 1 &&
            BigInt(42) === 42n && BigInt("0x10") === 16n && BigInt(true) === 1n &&
            BigInt.asUintN(8, -1n) === 255n && BigInt.asIntN(8, 255n) === -1n &&
            BigInt.asUintN(Number.MAX_SAFE_INTEGER, 1n) === 1n &&
            BigInt.asIntN(Number.MAX_SAFE_INTEGER, -1n) === -1n &&
            [10n].toString() === "10" && BigInt([10n]) === 10n &&
            (255n).toString(16) === "ff" && (42n).toLocaleString() === "42" &&
            BigInt.prototype.valueOf.call(boxed) === 255n &&
            boxed.valueOf() === 255n && boxed.constructor === BigInt &&
            Object.prototype.toString.call(boxed) === "[object BigInt]" &&
            Object.prototype.toString.call(BigInt.prototype) === "[object BigInt]" &&
            BigInt.prototype[Symbol.toStringTag] === "BigInt" &&
            String(9007199254740993n) === "9007199254740993" &&
            Number(42n) === 42 && Boolean(0n) === false && Boolean(1n) === true &&
            constructError ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn bigint_to_string_falls_back_to_object_for_a_non_string_tag() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        Object.defineProperty(BigInt.prototype, Symbol.toStringTag, { value: null });
        Object.prototype.toString.call(1n) === "[object Object]" &&
            Object.prototype.toString.call(Object(1n)) === "[object Object]"
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn moves_ownerless_bigints_across_vm_boundaries() -> TestResult {
    let engine = rs_quickjs::Engine::new();
    let mut first = engine.create_vm();
    let owned = first.eval_owned("9007199254740993n")?;
    ensure_owned_bigint(&owned, "9007199254740993")?;
    drop(first);

    let mut second = engine.create_vm();
    second.register_host_function_typed("hostBigInt", move |_call| Ok(owned.clone()))?;
    let result = second.eval_owned("hostBigInt() + 7n")?;
    ensure_owned_bigint(&result, "9007199254741000")
}

#[test]
fn enforces_result_bit_limits_before_expensive_materialization() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_bigint_bits: 64,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    for source in [
        "1n << 1000n",
        "2n ** 1000n",
        "BigInt.asUintN(1000, -1n)",
        "BigInt('340282366920938463463374607431768211455')",
    ] {
        if context.eval(source).is_ok() {
            return Err(format!("expected BigInt bit limit to reject {source}").into());
        }
    }
    let value =
        context.eval("0n << 1000000000000n === 0n && 1n ** 1000000000000n === 1n ? 42 : 0")?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_owned_bigint(actual: &OwnedValue, expected: &str) -> TestResult {
    let OwnedValue::BigInt(actual) = actual else {
        return Err(format!("expected BigInt {expected}, got {actual:?}").into());
    };
    if actual.to_string() == expected {
        return Ok(());
    }
    Err(format!("expected BigInt {expected}, got {actual}").into())
}
