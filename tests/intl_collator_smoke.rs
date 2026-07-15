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
fn compares_with_sensitivity_and_punctuation_options() -> TestResult {
    ensure_true(&eval(
        r#"
        const base = new Intl.Collator("en", { sensitivity: "base" });
        const punctuation = new Intl.Collator("en", {
            ignorePunctuation: true
        });
        const numeric = new Intl.Collator("en", { numeric: true });
        base.compare("Aã", "Aa") === 0 &&
            punctuation.compare("A-B", "AB") === 0 &&
            numeric.compare("item9", "item10") < 0 &&
            base.compare === base.compare
        "#,
    )?)
}

#[test]
fn resolves_unicode_extensions_and_supported_locales() -> TestResult {
    ensure_true(&eval(
        r#"
        const options = new Intl.Collator("de-u-co-phonebk-kf-lower-kn", {
            numeric: true
        }).resolvedOptions();
        options.locale === "de-u-co-phonebk-kf-lower-kn" &&
            options.collation === "phonebk" &&
            options.numeric === true &&
            options.caseFirst === "lower" &&
            Intl.Collator.supportedLocalesOf(["de", "zxx"]).join() === "de"
        "#,
    )?)
}

#[test]
fn locale_string_methods_use_intl_locale_semantics() -> TestResult {
    ensure_true(&eval(
        r#"
        "Iİ".toLocaleLowerCase("tr") === "ıi" &&
            "iı".toLocaleUpperCase("tr") === "İI" &&
            "a".localeCompare("A", "en") < 0 &&
            "a".localeCompare("A", "en") ===
                new Intl.Collator("en").compare("a", "A")
        "#,
    )?)
}

#[test]
fn supported_values_cover_each_advertised_intl_domain() -> TestResult {
    ensure_true(&eval(
        r#"
        const collations = Intl.supportedValuesOf("collation");
        const currencies = Intl.supportedValuesOf("currency");
        const timeZones = Intl.supportedValuesOf("timeZone");
        const units = Intl.supportedValuesOf("unit");
        collations.includes("eor") && collations.includes("phonebk") &&
            currencies.includes("USD") && currencies.includes("EUR") &&
            timeZones.includes("Etc/GMT+12") && timeZones.includes("Etc/GMT-14") &&
            units.includes("meter") && units.includes("year")
        "#,
    )?)
}
