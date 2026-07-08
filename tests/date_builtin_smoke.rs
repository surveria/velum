use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_date_constructor_and_utc_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let epoch = new Date(0);
        let parsed = Date.parse("2020-01-02T03:04:05.006Z");
        let utc = Date.UTC(2020, 0, 2, 3, 4, 5, 6);
        let constructed = new Date(2020, 0, 2, 3, 4, 5, 6);
        let fromString = new Date("2020-01-02T03:04:05.006Z");
        let invalid = new Date(NaN);
        let mutable = new Date(0);
        let setResult = mutable.setTime(1000);
        let originalPrototype = Date.prototype;
        Date.prototype = null;

        print(epoch.toISOString(), epoch.toUTCString(), epoch.toDateString());
        print(
            fromString.getUTCFullYear(),
            fromString.getUTCMonth(),
            fromString.getUTCDate(),
            fromString.getUTCDay(),
            fromString.getUTCHours(),
            fromString.getUTCMinutes(),
            fromString.getUTCSeconds(),
            fromString.getUTCMilliseconds()
        );
        print(typeof Date(), typeof Date.now(), Date.name, Date.length);

        Date.prototype === originalPrototype &&
            Date.prototype.constructor === Date &&
            Date.now.length === 0 &&
            Date.parse.length === 1 &&
            Date.UTC.length === 7 &&
            epoch.getTime() === 0 &&
            epoch.valueOf() === 0 &&
            epoch.getFullYear() === 1970 &&
            epoch.getMonth() === 0 &&
            epoch.getDate() === 1 &&
            epoch.getDay() === 4 &&
            epoch.getHours() === 0 &&
            epoch.getMinutes() === 0 &&
            epoch.getSeconds() === 0 &&
            epoch.getMilliseconds() === 0 &&
            parsed === utc &&
            constructed.toISOString() === "2020-01-02T03:04:05.006Z" &&
            fromString.toJSON() === "2020-01-02T03:04:05.006Z" &&
            invalid.toString() === "Invalid Date" &&
            invalid.toJSON() === null &&
            invalid.getTime() !== invalid.getTime() &&
            Date.parse("not a date") !== Date.parse("not a date") &&
            setResult === 1000 &&
            mutable.getTime() === 1000 &&
            mutable.toISOString() === "1970-01-01T00:00:01.000Z" &&
            Date.prototype.getTime() !== Date.prototype.getTime()
            ? 42
            : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "1970-01-01T00:00:00.000Z Thu, 01 Jan 1970 00:00:00 GMT Thu Jan 01 1970".to_owned(),
            "2020 0 2 4 3 4 5 6".to_owned(),
            "string number Date 7".to_owned(),
        ],
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

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::String(expected.to_owned()) {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {value:?}").into())
}

#[test]
fn component_constructor_and_getters_agree() -> TestResult {
    ensure_string(
        r#"
        const d = new Date(2020, 5, 15, 10, 30, 45, 500);
        "" + d.getFullYear() + ":" + d.getMonth() + ":" + d.getDate()
            + ":" + d.getHours() + ":" + d.getMinutes() + ":" + d.getSeconds()
            + ":" + d.getMilliseconds()
        "#,
        "2020:5:15:10:30:45:500",
    )
}

#[test]
fn copy_and_millisecond_forms_share_time_values() -> TestResult {
    ensure_string(
        r#"
        const base = new Date(86400000);
        const copy = new Date(base);
        "" + base.getTime() + ":" + copy.getTime() + ":" + (base === copy)
        "#,
        "86400000:86400000:false",
    )
}

#[test]
fn invalid_dates_propagate_nan() -> TestResult {
    ensure_string(
        r#"
        const bad = new Date(NaN);
        const overflow = new Date(8.65e15);
        "" + (bad.getTime() !== bad.getTime())
            + ":" + (overflow.getTime() !== overflow.getTime())
        "#,
        "true:true",
    )
}

#[test]
fn iso_round_trip_preserves_time() -> TestResult {
    ensure_string(
        r#"
        const iso = "2020-01-02T03:04:05.006Z";
        const parsed = new Date(Date.parse(iso));
        parsed.toISOString() + ":" + (parsed.getTime() === new Date(iso).getTime())
        "#,
        "2020-01-02T03:04:05.006Z:true",
    )
}

#[test]
fn set_time_mutates_and_returns_value() -> TestResult {
    ensure_string(
        r#"
        const d = new Date(0);
        const returned = d.setTime(123456);
        "" + d.getTime() + ":" + returned
        "#,
        "123456:123456",
    )
}

#[test]
fn prototype_identity_holds_for_dates() -> TestResult {
    ensure_string(
        r#"
        const d = new Date(0);
        "" + (Object.getPrototypeOf(d) === Date.prototype)
            + ":" + (d instanceof Date)
            + ":" + (typeof d.toJSON === "function")
        "#,
        "true:true:true",
    )
}
