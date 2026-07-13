use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_true(value: &Value) -> TestResult {
    if value == &Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}

#[test]
fn formats_digital_durations_and_parts() -> TestResult {
    ensure_true(&eval(
        r#"
        const formatter = new Intl.DurationFormat("en", {
            style: "digital",
            fractionalDigits: 4
        });
        const duration = {
            days: 2,
            hours: 1,
            minutes: 2,
            seconds: 3,
            milliseconds: 456
        };
        const parts = formatter.formatToParts(duration);
        formatter.format(duration) === "2 days, 1:02:03.4560" &&
            parts.map((part) => part.value).join("") === "2 days, 1:02:03.4560" &&
            parts.filter((part) => part.unit === "second").length === 3
        "#,
    )?)
}

#[test]
fn resolves_options_and_supported_locales() -> TestResult {
    ensure_true(&eval(
        r#"
        const options = new Intl.DurationFormat("en-u-nu-arab", {
            hours: "numeric",
            fractionalDigits: 3
        }).resolvedOptions();
        options.locale === "en-u-nu-arab" &&
            options.numberingSystem === "arab" &&
            options.hours === "numeric" &&
            options.minutes === "2-digit" &&
            options.seconds === "2-digit" &&
            options.fractionalDigits === 3 &&
            Intl.DurationFormat.supportedLocalesOf(["en", "zxx"]).join() === "en"
        "#,
    )?)
}
