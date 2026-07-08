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

#[test]
fn setter_families_update_components() -> TestResult {
    ensure_string(
        r#"
        const d = new Date(0);
        const fullYear = d.setFullYear(2001, 1, 3);
        const month = d.setMonth(2, 4);
        const date = d.setDate(5);
        const hours = d.setHours(6, 7, 8, 9);
        const minutes = d.setMinutes(10, 11, 12);
        const seconds = d.setSeconds(13, 14);
        const milliseconds = d.setMilliseconds(15);
        const utc = new Date(0);
        const utcFullYear = utc.setUTCFullYear(2022, 11, 31);
        utc.setUTCMonth(0, 2);
        utc.setUTCDate(3);
        utc.setUTCHours(4, 5, 6, 7);
        utc.setUTCMinutes(8, 9, 10);
        utc.setUTCSeconds(11, 12);
        utc.setUTCMilliseconds(13);

        [
            d.toISOString(),
            utc.toISOString(),
            fullYear === Date.UTC(2001, 1, 3),
            month === Date.UTC(2001, 2, 4),
            date === Date.UTC(2001, 2, 5),
            hours === Date.UTC(2001, 2, 5, 6, 7, 8, 9),
            minutes === Date.UTC(2001, 2, 5, 6, 10, 11, 12),
            seconds === Date.UTC(2001, 2, 5, 6, 10, 13, 14),
            milliseconds === Date.UTC(2001, 2, 5, 6, 10, 13, 15),
            utcFullYear === Date.UTC(2022, 11, 31)
        ].join("|")
        "#,
        "2001-03-05T06:10:13.015Z|2022-01-03T04:08:11.013Z|true|true|true|true|true|true|true|true",
    )
}

#[test]
fn setters_handle_invalid_dates_and_timezone_offset() -> TestResult {
    ensure_string(
        r#"
        const valid = new Date(0);
        const invalid = new Date(NaN);
        const restored = invalid.setFullYear(2020);
        const stillInvalid = new Date(NaN);
        const failed = stillInvalid.setMonth(1);
        [
            valid.getTimezoneOffset(),
            new Date(NaN).getTimezoneOffset() !== new Date(NaN).getTimezoneOffset(),
            restored === Date.UTC(2020, 0, 1),
            invalid.toISOString(),
            failed !== failed,
            stillInvalid.getTime() !== stillInvalid.getTime()
        ].join("|")
        "#,
        "0|true|true|2020-01-01T00:00:00.000Z|true|true",
    )
}

#[test]
fn symbol_to_primitive_uses_date_ordering() -> TestResult {
    ensure_string(
        r#"
        const method = Date.prototype[Symbol.toPrimitive];
        const d = new Date(0);
        let order = "";
        const ordinary = {
            toString() { order += "s"; return "ordinary"; },
            valueOf() { order += "v"; return 7; }
        };
        const defaultValue = method.call(d, "default");
        const stringValue = method.call(d, "string");
        const numberValue = method.call(d, "number");
        const ordinaryDefault = method.call(ordinary, "default");
        const defaultOrder = order;
        order = "";
        const ordinaryNumber = method.call(ordinary, "number");
        [
            method.name,
            method.length,
            defaultValue === d.toString(),
            stringValue === d.toString(),
            numberValue,
            ordinaryDefault,
            defaultOrder,
            ordinaryNumber,
            order
        ].join("|")
        "#,
        "[Symbol.toPrimitive]|1|true|true|0|ordinary|s|7|v",
    )
}

#[test]
fn annex_b_and_locale_date_methods_are_available() -> TestResult {
    ensure_string(
        r#"
        const d = new Date(0);
        const setYearReturn = d.setYear(99);
        const invalid = new Date(NaN);
        const invalidSetYear = invalid.setYear();
        [
            new Date(0).getYear(),
            setYearReturn,
            d.toISOString(),
            invalidSetYear !== invalidSetYear,
            invalid.getTime() !== invalid.getTime(),
            Date.prototype.toGMTString === Date.prototype.toUTCString,
            Date.prototype.toGMTString.name,
            Date.prototype.toGMTString.length,
            d.toLocaleString(),
            d.toLocaleDateString(),
            d.toLocaleTimeString(),
            Date.prototype.toLocaleString.name,
            Date.prototype.toLocaleDateString.name,
            Date.prototype.toLocaleTimeString.name
        ].join("|")
        "#,
        concat!(
            "70|915148800000|1999-01-01T00:00:00.000Z|true|true|true|toUTCString|0|",
            "Fri Jan 01 1999 00:00:00 GMT+0000 (UTC)|Fri Jan 01 1999|",
            "00:00:00 GMT+0000 (UTC)|toLocaleString|toLocaleDateString|toLocaleTimeString"
        ),
    )
}
