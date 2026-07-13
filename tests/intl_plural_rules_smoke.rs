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
fn selects_categories_and_exposes_resolved_state() -> TestResult {
    ensure_true(&eval(
        r#"
        const cardinal = new Intl.PluralRules("en-US");
        const ordinal = new Intl.PluralRules("en-US", { type: "ordinal" });
        const options = cardinal.resolvedOptions();
        cardinal.select(1) === "one" &&
            cardinal.select(2) === "other" &&
            ordinal.select(2) === "two" &&
            cardinal.selectRange(1, 2) === "other" &&
            options.locale === "en-US" &&
            options.type === "cardinal" &&
            options.notation === "standard" &&
            options.pluralCategories.join() === "one,other" &&
            Intl.PluralRules.supportedLocalesOf(["en-US", "zxx"]).join() ===
                "en-US"
        "#,
    )?)
}

#[test]
fn applies_french_notation_rules() -> TestResult {
    ensure_true(&eval(
        r#"
        const standard = new Intl.PluralRules("fr", { notation: "standard" });
        const compact = new Intl.PluralRules("fr", { notation: "compact" });
        standard.select(1e6) === "many" &&
            standard.select(1.5e6) === "other" &&
            compact.select(1.5e6) === "many" &&
            compact.resolvedOptions().compactDisplay === "short"
        "#,
    )?)
}
