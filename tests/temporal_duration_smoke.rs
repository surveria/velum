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
fn exposes_duration_constructor_fields_and_metadata() -> TestResult {
    let value = eval(
        r#"
        const duration = new Temporal.Duration(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
        const descriptor = Object.getOwnPropertyDescriptor(
            Temporal.Duration.prototype,
            "years"
        );
        Temporal.Duration.length === 0 &&
            Temporal.Duration.from.length === 1 &&
            Temporal.Duration.compare.length === 2 &&
            descriptor.enumerable === false &&
            descriptor.configurable === true &&
            duration.years === 1 &&
            duration.months === 2 &&
            duration.weeks === 3 &&
            duration.days === 4 &&
            duration.hours === 5 &&
            duration.minutes === 6 &&
            duration.seconds === 7 &&
            duration.milliseconds === 8 &&
            duration.microseconds === 9 &&
            duration.nanoseconds === 10
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn parses_clones_and_combines_durations() -> TestResult {
    let value = eval(
        r#"
        const parsed = Temporal.Duration.from("PT1.5S");
        const cloned = Temporal.Duration.from(parsed);
        const combined = new Temporal.Duration(0, 0, 0, 0, 1)
            .add({ minutes: 30 })
            .subtract({ minutes: 15 });
        parsed !== cloned &&
            parsed.toString() === "PT1.5S" &&
            combined.toString() === "PT1H15M" &&
            combined.negated().abs().toString() === "PT1H15M" &&
            Temporal.Duration.compare(parsed, cloned) === 0
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn rejects_invalid_duration_receivers_and_fields() -> TestResult {
    let value = eval(
        r#"
        let failures = 0;
        const years = Object.getOwnPropertyDescriptor(
            Temporal.Duration.prototype,
            "years"
        ).get;
        for (const callback of [
            () => Temporal.Duration(),
            () => new Temporal.Duration(0.5),
            () => Temporal.Duration.from({}),
            () => years.call(Temporal.Duration.prototype),
            () => Temporal.Duration.prototype.toString(),
        ]) {
            try {
                callback();
            } catch (error) {
                if (error instanceof TypeError || error instanceof RangeError) failures += 1;
            }
        }
        failures
        "#,
    )?;
    ensure_value(&value, &Value::Number(5.0))
}
