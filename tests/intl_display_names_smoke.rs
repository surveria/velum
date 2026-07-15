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
fn validates_codes_and_exposes_resolved_state() -> TestResult {
    ensure_true(&eval(
        r#"
        const names = new Intl.DisplayNames("en-US", {
            type: "language",
            style: "short",
            languageDisplay: "standard"
        });
        const options = names.resolvedOptions();
        names instanceof Intl.DisplayNames &&
            typeof names.of("fr-Latn-FR") === "string" &&
            options.locale === "en-US" &&
            options.style === "short" &&
            options.type === "language" &&
            options.fallback === "code" &&
            options.languageDisplay === "standard" &&
            Object.keys(options).join() ===
                "locale,style,type,fallback,languageDisplay"
        "#,
    )?)
}

#[test]
fn rejects_invalid_codes_and_non_object_options() -> TestResult {
    ensure_true(&eval(
        r#"
        let invalidCode = false;
        let invalidOptions = false;
        try {
            new Intl.DisplayNames("en", { type: "region" }).of("region");
        } catch (error) {
            invalidCode = error instanceof RangeError;
        }
        try {
            new Intl.DisplayNames("en", "language");
        } catch (error) {
            invalidOptions = error instanceof TypeError;
        }
        invalidCode && invalidOptions
        "#,
    )?)
}
