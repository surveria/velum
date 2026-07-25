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
fn segments_graphemes_words_and_sentences() -> TestResult {
    ensure_true(&eval(
        r#"
        const graphemes = [...new Intl.Segmenter("en", {
            granularity: "grapheme"
        }).segment("a\u0301b")];
        const words = [...new Intl.Segmenter("en", {
            granularity: "word"
        }).segment("hello world")];
        const sentences = [...new Intl.Segmenter("en", {
            granularity: "sentence"
        }).segment("One. Two!")];
        graphemes.length === 2 &&
            graphemes[0].segment === "a\u0301" &&
            graphemes[1].index === 2 &&
            words.map((part) => part.segment).join("") === "hello world" &&
            words.filter((part) => part.isWordLike).length === 2 &&
            sentences.map((part) => part.segment).join("") === "One. Two!"
        "#,
    )?)
}

#[test]
fn containing_and_iterators_preserve_utf16_boundaries() -> TestResult {
    ensure_true(&eval(
        r#"
        const input = "\ud83d\udc4b\ud83c\udffb x";
        const segments = new Intl.Segmenter(undefined, {
            granularity: "grapheme"
        }).segment(input);
        const emoji = segments.containing(2);
        const first = segments[Symbol.iterator]();
        const second = segments[Symbol.iterator]();
        emoji.segment === "\ud83d\udc4b\ud83c\udffb" &&
            emoji.index === 0 &&
            emoji.input === input &&
            first !== second &&
            first.next().value.segment === second.next().value.segment &&
            segments.containing(input.length) === undefined
        "#,
    )?)
}

#[test]
fn exposes_resolved_options_and_supported_locales() -> TestResult {
    ensure_true(&eval(
        r#"
        const segmenter = new Intl.Segmenter(["xyz", "ar"], {
            granularity: "word"
        });
        const options = segmenter.resolvedOptions();
        options.locale === "ar" &&
            options.granularity === "word" &&
            Object.keys(options).join() === "locale,granularity" &&
            Intl.Segmenter.supportedLocalesOf(["de", "zxx"]).join() === "de"
        "#,
    )?)
}

#[test]
fn rejects_non_object_options() -> TestResult {
    ensure_true(&eval(
        r#"
        let constructorRejected = false;
        try {
            new Intl.Segmenter(Intl.Segmenter, 24021);
        } catch (error) {
            constructorRejected = error instanceof TypeError;
        }
        let reads = [];
        Object.defineProperty(Object.prototype, "localeMatcher", {
            configurable: true,
            get() {
                reads.push("localeMatcher");
                return "best fit";
            }
        });
        const supported = Intl.Segmenter.supportedLocalesOf(["en"], 24021);
        delete Object.prototype.localeMatcher;
        constructorRejected &&
            supported.join() === "en" &&
            reads.join() === "localeMatcher"
        "#,
    )?)
}

#[test]
fn supported_locales_options_null_is_rejected() -> TestResult {
    ensure_true(&eval(
        r#"
        let rejected = false;
        try {
            Intl.Segmenter.supportedLocalesOf(["en"], null);
        } catch (error) {
            rejected = error instanceof TypeError;
        }
        rejected
        "#,
    )?)
}
