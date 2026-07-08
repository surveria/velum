use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const MATH_BUILTIN_SCRIPT: &str = r#"
        let mathObject = Math;
        let keys = "";
        for (let key in Math) {
            keys = keys + key + ";";
        }

        let shadow = 0;
        {
            let Math = {
                abs: function(value) {
                    return value + 35;
                }
            };
            shadow = Math.abs(7);
        }

        let nanAbs = Math.abs();
        let maxNaN = Math.max(1, NaN);
        let minNaN = Math.min(NaN, 1);
        let maxPositiveZero = 1 / Math.max(-0, 0);
        let minNegativeZero = 1 / Math.min(0, -0);
        let deleteMath = delete Math;

        print(
            typeof Math,
            Math.__proto__ === Object.prototype,
            Math.PI > 3.14,
            Math.E > 2.71,
            Math.abs.name,
            Math.max.length,
            Math.pow.length
        );
        print(
            Math.abs(-7),
            Math.ceil(1.2),
            Math.floor(1.8),
            Math.trunc(-1.8),
            Math.round(1.5),
            Math.round(-1.5),
            Math.sqrt(81),
            Math.pow(2, 5),
            Math.max(1, 7, 3),
            Math.min(1, -2, 3)
        );
        print(
            Math.max(),
            Math.min(),
            nanAbs !== nanAbs,
            maxNaN !== maxNaN,
            minNaN !== minNaN
        );
        print(maxPositiveZero === Infinity, minNegativeZero === -Infinity, "keys:" + keys, shadow);

        mathObject === Math &&
            typeof Math === "object" &&
            Math.__proto__ === Object.prototype &&
            typeof Math.abs === "function" &&
            Math.abs.name === "abs" &&
            Math.abs.length === 1 &&
            Math.max.length === 2 &&
            Math.pow.length === 2 &&
            Math.PI > 3.14 &&
            Math.E > 2.71 &&
            Math.LN10 > 2.30 &&
            Math.LN2 > 0.69 &&
            Math.LOG10E > 0.43 &&
            Math.LOG2E > 1.44 &&
            Math.SQRT1_2 > 0.70 &&
            Math.SQRT2 > 1.41 &&
            Math.abs(-7) === 7 &&
            Math.ceil(1.2) === 2 &&
            Math.floor(1.8) === 1 &&
            Math.trunc(-1.8) === -1 &&
            Math.round(1.5) === 2 &&
            Math.round(-1.5) === -1 &&
            Math.sqrt(81) === 9 &&
            Math.pow(2, 5) === 32 &&
            Math.max(1, 7, 3) === 7 &&
            Math.min(1, -2, 3) === -2 &&
            Math.max() === -Infinity &&
            Math.min() === Infinity &&
            nanAbs !== nanAbs &&
            maxNaN !== maxNaN &&
            minNaN !== minNaN &&
            maxPositiveZero === Infinity &&
            minNegativeZero === -Infinity &&
            deleteMath === false &&
            keys === "" &&
            shadow === 42 ? 42 : 0
"#;

const MATH_RESIDUAL_SCRIPT: &str = r#"
        let constantDescriptor = Object.getOwnPropertyDescriptor(Math, "PI");
        let tagDescriptor = Object.getOwnPropertyDescriptor(Math, Symbol.toStringTag);
        let globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "Math");
        let f16Descriptor = Object.getOwnPropertyDescriptor(Math, "f16round");
        let sumDescriptor = Object.getOwnPropertyDescriptor(Math, "sumPrecise");

        let f16Ok =
            Math.f16round(1.1) === 1.099609375 &&
            Math.f16round(2049) === 2048 &&
            Math.f16round(2051) === 2052 &&
            Math.f16round(65520) === Infinity &&
            Math.f16round(65519.99999999999) === 65504 &&
            Math.f16round(2.9802322387695312e-8) === 0 &&
            Math.f16round(2.980232238769532e-8) === 5.960464477539063e-8 &&
            1 / Math.f16round(-0) === -Infinity;

        let roundOk =
            1 / Math.round(-0.5) === -Infinity &&
            1 / Math.round(0.5 - Number.EPSILON / 4) === Infinity &&
            Math.round(2 / Number.EPSILON - 1) === 2 / Number.EPSILON - 1;

        let sumOk =
            Object.is(Math.sumPrecise([]), -0) &&
            Object.is(Math.sumPrecise([-0]), -0) &&
            Math.sumPrecise([-0, 0]) === 0 &&
            Math.sumPrecise([1, 2, 3]) === 6 &&
            Math.sumPrecise([1e30, 0.1, -1e30]) === 0.1 &&
            Math.sumPrecise([Infinity, -Infinity]) !== Math.sumPrecise([Infinity, -Infinity]) &&
            Math.sumPrecise([Infinity]) === Infinity;

        let descriptorsOk =
            constantDescriptor.value === Math.PI &&
            constantDescriptor.writable === false &&
            constantDescriptor.enumerable === false &&
            constantDescriptor.configurable === false &&
            Math[Symbol.toStringTag] === "Math" &&
            tagDescriptor.writable === false &&
            tagDescriptor.enumerable === false &&
            tagDescriptor.configurable === true &&
            globalDescriptor.value === Math &&
            globalDescriptor.writable === true &&
            globalDescriptor.enumerable === false &&
            globalDescriptor.configurable === true &&
            Math.f16round.name === "f16round" &&
            Math.f16round.length === 1 &&
            f16Descriptor.writable === true &&
            f16Descriptor.enumerable === false &&
            f16Descriptor.configurable === true &&
            Math.sumPrecise.name === "sumPrecise" &&
            Math.sumPrecise.length === 1 &&
            sumDescriptor.writable === true &&
            sumDescriptor.enumerable === false &&
            sumDescriptor.configurable === true;

        descriptorsOk && f16Ok && roundOk && sumOk ? 42 : 0
"#;

#[test]
fn exposes_math_object_constants_methods_and_shadowing() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(MATH_BUILTIN_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "object true true true abs 2 2",
            "7 2 1 -1 2 -1 9 32 7 -2",
            "-Infinity Infinity true true true",
            "true true keys: 42",
        ],
    )
}

#[test]
fn exposes_residual_math_descriptors_and_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(MATH_RESIDUAL_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
