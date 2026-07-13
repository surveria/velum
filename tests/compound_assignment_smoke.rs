use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_compound_assignment_targets() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let value = 10;
        let add = value += 5;
        let sub = value -= 3;
        let mul = value *= 4;
        let div = value /= 2;
        let rem = value %= 7;
        let mask = value &= 6;
        print(add, sub, mul, div, rem, mask, value);

        let label = "cam";
        label += "-01";
        print(label);

        let sensor = { count: 10 };
        let propAdd = sensor.count += 5;
        let propSub = sensor.count -= 3;
        print(propAdd, propSub, sensor.count);

        let values = [1, 2, 3];
        let index = 1;
        let cellMul = values[index] *= 5;
        let cellBit = values[index] &= 6;
        print(cellMul, cellBit, values[1]);

        let order = "";
        let target = { slot: 40 };
        let key = function() {
            order += "k";
            return "slot";
        };
        let rhs = function() {
            order += "r";
            return 2;
        };
        let ordered = target[key()] += rhs();
        print(order, ordered, target.slot);

        add === 15 &&
            sub === 12 &&
            mul === 48 &&
            div === 24 &&
            rem === 3 &&
            mask === 2 &&
            value === 2 &&
            label === "cam-01" &&
            propAdd === 15 &&
            propSub === 12 &&
            sensor.count === 12 &&
            cellMul === 10 &&
            cellBit === 2 &&
            values[1] === 2 &&
            order === "kr" &&
            ordered === 42 &&
            target.slot === 42
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(
        context.output(),
        &[
            "15 12 48 24 3 2 2".to_owned(),
            "cam-01".to_owned(),
            "15 12 12".to_owned(),
            "10 2 2".to_owned(),
            "kr 42 42".to_owned(),
        ],
    )
}

#[test]
fn rejects_invalid_compound_assignment_targets() -> TestResult {
    ensure_error_contains(
        "let value = 1; (value + 1) += 2",
        "invalid assignment target",
    )?;
    ensure_error_contains("const fixed = 1; fixed += 1", "assignment to constant")?;
    ensure_source_value(
        r#"let value = "camera"; value -= 1; Number.isNaN(value)"#,
        &Value::Bool(true),
    )
}

#[test]
fn checks_compound_assignment_reference_before_rhs() -> TestResult {
    ensure_source_value(
        r#"
        let rhs = false;
        try {
            let base = null;
            let key = { toString() { throw "key"; } };
            base[key] += (rhs = true, 1);
        } catch (error) {
            error instanceof TypeError && rhs === false;
        }
        "#,
        &Value::Bool(true),
    )
}

#[test]
fn supports_extended_compound_assignment_operators() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let value = 5;
        let orValue = value |= 2;
        let xorValue = value ^= 3;
        let left = value <<= 2;
        let right = value >>= 1;
        let unsigned = value >>>= 1;
        let power = value **= 3;
        print(orValue, xorValue, left, right, unsigned, power, value);

        let negative = -8;
        let signed = negative >>= 1;
        let unsignedRight = negative >>>= 1;
        print(signed, unsignedRight, negative);

        let flags = { mask: 1 };
        let propOr = flags.mask |= 4;
        let propXor = flags.mask ^= 7;
        let propShift = flags.mask <<= 3;
        print(propOr, propXor, propShift, flags.mask);

        let values = [1, 8];
        let cellLeft = values[0] <<= 5;
        let cellRight = values[1] >>= 2;
        let cellPower = values[1] **= 5;
        print(cellLeft, cellRight, cellPower, values[0], values[1]);

        let order = '';
        let target = { slot: 32 };
        let key = function() {
            order += 'k';
            return 'slot';
        };
        let rhs = function() {
            order += 'r';
            return 10;
        };
        let ordered = target[key()] ^= rhs();
        print(order, ordered, target.slot);

        let binary =
            (1 | 4) +
            (7 ^ 3) +
            (1 << 4) +
            (16 >> 2) +
            (-1 >>> 30) +
            (2 ** 3 ** 2);

        orValue === 7 &&
            xorValue === 4 &&
            left === 16 &&
            right === 8 &&
            unsigned === 4 &&
            power === 64 &&
            value === 64 &&
            signed === -4 &&
            unsignedRight === 2147483646 &&
            negative === 2147483646 &&
            propOr === 5 &&
            propXor === 2 &&
            propShift === 16 &&
            flags.mask === 16 &&
            cellLeft === 32 &&
            cellRight === 2 &&
            cellPower === 32 &&
            values[0] === 32 &&
            values[1] === 32 &&
            order === 'kr' &&
            ordered === 42 &&
            target.slot === 42 &&
            binary === 544
        ",
    )?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(
        context.output(),
        &[
            "7 4 16 8 4 64 64".to_owned(),
            "-4 2147483646 2147483646".to_owned(),
            "5 2 16 16".to_owned(),
            "32 2 32 32 32".to_owned(),
            "kr 42 42".to_owned(),
        ],
    )?;
    ensure_source_value(
        r#"let value = "camera"; value **= 2; Number.isNaN(value)"#,
        &Value::Bool(true),
    )
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

fn ensure_source_value(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    ensure_value(&actual, expected)
}

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
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
