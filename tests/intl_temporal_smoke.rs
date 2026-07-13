use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

#[test]
fn exposes_intl_temporal_formatters() -> TestResult {
    let value = eval(
        r#"
        const dateTime = new Intl.DateTimeFormat("en", { timeZone: "UTC" });
        const duration = new Intl.DurationFormat("en");
        dateTime instanceof Intl.DateTimeFormat &&
            duration instanceof Intl.DurationFormat &&
            Intl.DateTimeFormat.length === 0 &&
            Intl.DurationFormat.length === 0 &&
            typeof Intl.Collator === "function" &&
            typeof Intl.NumberFormat === "function" &&
            typeof Intl.PluralRules === "function" &&
            typeof Intl.RelativeTimeFormat === "function" &&
            Intl.supportedValuesOf("calendar").includes("gregory") &&
            Intl.supportedValuesOf("timeZone").includes("UTC")
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn formats_temporal_values_and_parts() -> TestResult {
    let value = eval(
        r#"
        const instant = new Temporal.Instant(1735213600_321_000_000n);
        const options = {
            timeZone: "UTC",
            year: "numeric",
            month: "numeric",
            day: "numeric",
            hour: "numeric",
            minute: "numeric",
            second: "numeric"
        };
        const formatter = new Intl.DateTimeFormat("en", options);
        const formatted = formatter.format(instant);
        const parts = formatter.formatToParts(instant);
        formatted === instant.toLocaleString("en", options) &&
            formatted.includes("2024") &&
            formatted.includes("11:46:40") &&
            parts.some((part) => part.type === "year" && part.value === "2024") &&
            parts.some((part) => part.type === "second" && part.value === "40")
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn exposes_date_time_format_methods() -> TestResult {
    let value = eval(
        r#"
        const formatter = new Intl.DateTimeFormat("en-US", { timeZone: "UTC" });
        const first = formatter.format;
        const second = formatter.format;
        const parts = formatter.formatRangeToParts(0, 0);
        first === second &&
            first(0) === formatter.format(0) &&
            formatter.formatRange(0, 0) === formatter.format(0) &&
            parts.length > 0 &&
            parts.every((part) => part.source === "shared") &&
            Intl.DateTimeFormat.supportedLocalesOf(["en-US"])[0] === "en-US"
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn formats_duration_like_values_consistently() -> TestResult {
    let value = eval(
        r#"
        const durationLike = { years: 1, months: 2, days: 3, hours: 4 };
        const duration = Temporal.Duration.from(durationLike);
        const formatter = new Intl.DurationFormat("en", { style: "long" });
        formatter.format(durationLike) === duration.toLocaleString("en", { style: "long" })
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}
