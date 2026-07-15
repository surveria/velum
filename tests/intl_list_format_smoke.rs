use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
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
fn formats_lists_and_exposes_resolved_state() -> TestResult {
    ensure_true(&eval(
        r#"
        const formatter = new Intl.ListFormat("en-US");
        const options = formatter.resolvedOptions();
        const parts = formatter.formatToParts(["one", "two", "three"]);
        formatter instanceof Intl.ListFormat &&
            formatter.format(["one", "two", "three"]) === "one, two, and three" &&
            parts.map((part) => part.value).join("") === "one, two, and three" &&
            parts.filter((part) => part.type === "element").length === 3 &&
            options.locale === "en-US" &&
            options.type === "conjunction" &&
            options.style === "long" &&
            Intl.ListFormat.supportedLocalesOf(["en-US", "zxx"]).join() === "en-US"
        "#,
    )?)
}

#[test]
fn closes_iterators_after_non_string_values() -> TestResult {
    ensure_true(&eval(
        r"
        let closed = false;
        const values = {
            [Symbol.iterator]() {
                return {
                    next() { return { value: 1, done: false }; },
                    return() {
                        closed = true;
                        return {};
                    }
                };
            }
        };
        let typeError = false;
        try {
            new Intl.ListFormat().format(values);
        } catch (error) {
            typeError = error instanceof TypeError;
        }
        typeError && closed
        ",
    )?)
}
