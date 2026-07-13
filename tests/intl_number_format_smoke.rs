use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_true(value: Value) -> TestResult {
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}

#[test]
fn exposes_number_format_state_and_methods() -> TestResult {
    ensure_true(eval(
        r#"
        const formatter = new Intl.NumberFormat("en-US", {
            style: "currency",
            currency: "usd",
            minimumFractionDigits: 2,
            maximumFractionDigits: 2,
            signDisplay: "always"
        });
        const options = formatter.resolvedOptions();
        const format = formatter.format;
        formatter instanceof Intl.NumberFormat &&
            format === formatter.format &&
            format.name === "" &&
            format.length === 1 &&
            format(1234.5) === "+$1,234.50" &&
            options.locale === "en-US" &&
            options.style === "currency" &&
            options.currency === "USD" &&
            options.minimumFractionDigits === 2 &&
            options.maximumFractionDigits === 2 &&
            options.useGrouping === "auto" &&
            Intl.NumberFormat.supportedLocalesOf(["en-US", "zxx"]).join() === "en-US"
        "#,
    )?)
}

#[test]
fn formats_parts_and_percent_values() -> TestResult {
    ensure_true(eval(
        r#"
        const formatter = new Intl.NumberFormat("en-US", {
            style: "percent",
            minimumFractionDigits: 1,
            maximumFractionDigits: 1
        });
        const parts = formatter.formatToParts(0.125);
        formatter.format(0.125) === "12.5%" &&
            parts.map((part) => part.value).join("") === "12.5%" &&
            parts.some((part) => part.type === "fraction" && part.value === "5") &&
            parts.some((part) => part.type === "percentSign" && part.value === "%")
        "#,
    )?)
}
