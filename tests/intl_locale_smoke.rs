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
fn canonicalizes_and_expands_non_iana_locales() -> TestResult {
    let value = eval(
        r#"
        const cases = [
            ["mo", "ro", "ro-Latn-RO", "ro"],
            ["es-ES-preeuro", "es-ES-preeuro", "es-Latn-ES-preeuro", "es-preeuro"],
            ["uz-UZ-cyrillic", "uz-UZ-cyrillic", "uz-Latn-UZ-cyrillic", "uz-cyrillic"],
            ["posix", "posix", "posix", "posix"],
            ["aar-x-private", "aa-x-private", "aa-Latn-ET-x-private", "aa-x-private"],
            ["heb-x-private", "he-x-private", "he-Hebr-IL-x-private", "he-x-private"],
            ["ces", "cs", "cs-Latn-CZ", "cs"],
            ["hy-arevela", "hy", "hy-Armn-AM", "hy"],
            ["hy-arevmda", "hyw", "hyw-Armn-AM", "hyw"]
        ];
        cases.every(([tag, canonical, maximal, minimal]) => {
            const locale = new Intl.Locale(tag);
            return locale.toString() === canonical &&
                locale.maximize().toString() === maximal &&
                locale.minimize().toString() === minimal;
        })
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn exposes_locale_options_accessors_and_information_methods() -> TestResult {
    let value = eval(
        r#"
        const locale = new Intl.Locale("de-Latn-DE-fonipa-1996", {
            calendar: "islamicc",
            collation: "phonebk",
            firstDayOfWeek: 3,
            hourCycle: "h23",
            caseFirst: "upper",
            numeric: true,
            numberingSystem: "latn"
        });
        const week = locale.getWeekInfo();
        const text = locale.getTextInfo();
        locale.toString() ===
            "de-Latn-DE-1996-fonipa-u-ca-islamic-civil-co-phonebk-fw-wed-hc-h23-kf-upper-kn-nu-latn" &&
            locale.baseName === "de-Latn-DE-1996-fonipa" &&
            locale.calendar === "islamic-civil" &&
            locale.firstDayOfWeek === "wed" &&
            locale.numeric === true &&
            week.firstDay === 3 &&
            Array.isArray(week.weekend) &&
            (text.direction === "ltr" || text.direction === "rtl") &&
            Array.isArray(locale.getCalendars()) &&
            Array.isArray(locale.getCollations()) &&
            Array.isArray(locale.getHourCycles()) &&
            Array.isArray(locale.getNumberingSystems())
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn canonical_locale_lists_use_locale_internal_slots() -> TestResult {
    let value = eval(
        r#"
        class PatchedLocale extends Intl.Locale {
            toString() {
                throw new Error("Locale internal slots must bypass toString");
            }
        }
        const locale = new PatchedLocale("fa");
        const values = Intl.getCanonicalLocales([new Intl.Locale("ar"), "zh", locale, "ar"]);
        values.join(",") === "ar,zh,fa" && Intl.getCanonicalLocales.length === 1
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}
