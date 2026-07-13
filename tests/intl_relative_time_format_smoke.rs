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
fn formats_relative_times_and_parts() -> TestResult {
    ensure_true(&eval(
        r#"
        const formatter = new Intl.RelativeTimeFormat("en-US", {
            numeric: "auto"
        });
        const parts = formatter.formatToParts(123456.78, "days");
        formatter.format(-1, "day") === "yesterday" &&
            formatter.format(0, "second") === "now" &&
            formatter.format(2, "weeks") === "in 2 weeks" &&
            parts.map((part) => part.value).join("") ===
                "in 123,456.78 days" &&
            parts.filter((part) => part.unit === "day").length === 5
        "#,
    )?)
}

#[test]
fn resolves_options_and_polish_patterns() -> TestResult {
    ensure_true(&eval(
        r#"
        const polish = new Intl.RelativeTimeFormat("pl-PL", { style: "short" });
        const arabicDigits = new Intl.RelativeTimeFormat("en-u-nu-arab");
        const options = arabicDigits.resolvedOptions();
        polish.format(2, "year") === "za 2 lata" &&
            polish.format(-10, "month") === "10 mies. temu" &&
            arabicDigits.format(12, "day").includes("١٢") &&
            options.locale === "en-u-nu-arab" &&
            options.numberingSystem === "arab" &&
            Intl.RelativeTimeFormat.supportedLocalesOf(["en", "zxx"]).join() ===
                "en"
        "#,
    )?)
}
