use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_strict_and_abstract_equality_contracts() -> TestResult {
    eval_is_42(
        r#"
        let shared = {};
        let symbol = Symbol("shared");
        let strict =
            NaN !== NaN &&
            0 === -0 &&
            "42" !== 42 &&
            shared === shared &&
            shared !== {} &&
            symbol === symbol &&
            symbol !== Symbol("shared");
        let abstract =
            null == undefined &&
            true == 1 &&
            false == "0" &&
            "42" == 42 &&
            new String("42") == 42 &&
            42 == new String("42") &&
            shared == shared &&
            shared != {};
        strict && abstract ? 42 : 0
        "#,
    )
}

#[test]
fn distinguishes_same_value_from_same_value_zero() -> TestResult {
    eval_is_42(
        r#"
        let shared = {};
        let symbol = Symbol("shared");
        Object.is(NaN, NaN) &&
            !Object.is(0, -0) &&
            Object.is(shared, shared) &&
            !Object.is(shared, {}) &&
            Object.is(symbol, symbol) &&
            !Object.is(symbol, Symbol("shared")) &&
            [NaN].includes(NaN) &&
            [NaN].indexOf(NaN) === -1 &&
            [-0].includes(0) &&
            [-0].indexOf(0) === 0 ? 42 : 0
        "#,
    )
}

#[test]
fn collections_share_same_value_zero() -> TestResult {
    eval_is_42(
        r"
        let map = new Map();
        map.set(NaN, 20);
        map.set(NaN, 40);
        map.set(-0, 41);
        map.set(0, 42);
        let set = new Set([NaN, NaN, -0, 0]);
        map.size === 2 &&
            map.get(NaN) === 40 &&
            map.get(-0) === 42 &&
            set.size === 2 &&
            set.has(NaN) &&
            set.has(-0) ? 42 : 0
        ",
    )
}

#[test]
fn optimized_numeric_paths_match_generic_equality() -> TestResult {
    eval_is_42(
        r"
        function equal(left, right) { return left === right; }
        let callback = [0, -0, NaN].map(function (value) {
            return equal(value, 0);
        });
        let masked = 0;
        for (let index = 0; index < 8; index += 1) {
            if ((index & 3) === 2) {
                masked += 1;
            }
        }
        let switched = 0;
        switch (-0) {
            case 0:
                switched = 40;
                break;
            default:
                switched = 0;
        }
        callback[0] === true &&
            callback[1] === true &&
            callback[2] === false &&
            masked === 2 &&
            switched + masked === 42 ? 42 : 0
        ",
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
