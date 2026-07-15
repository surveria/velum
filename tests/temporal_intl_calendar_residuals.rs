use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
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
fn resolves_calendar_era_fields_across_temporal_types() -> TestResult {
    let value = eval(
        r#"
        const fields = {
            calendar: "gregory",
            era: "bce",
            eraYear: 1,
            monthCode: "M12",
            day: 15,
        };
        const date = Temporal.PlainDate.from(fields);
        const dateTime = Temporal.PlainDateTime.from(fields);
        const monthDay = Temporal.PlainMonthDay.from(fields);
        const yearMonth = Temporal.PlainYearMonth.from(fields);
        const updated = date.with({ era: "ce", eraYear: 2 });
        date.year === 0 && date.era === "bce" && date.eraYear === 1 &&
            dateTime.year === 0 && dateTime.eraYear === 1 &&
            monthDay.calendarId === "gregory" &&
            yearMonth.year === 0 && yearMonth.eraYear === 1 &&
            updated.year === 2 && updated.era === "ce" && updated.eraYear === 2
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn rejects_intl_only_islamic_calendar_alias_in_temporal() -> TestResult {
    let value = eval(
        r#"
        const fields = { calendar: "islamic", year: 1500, month: 1, day: 1 };
        let failures = 0;
        for (const action of [
            () => Temporal.PlainDate.from(fields),
            () => Temporal.PlainDateTime.from(fields),
            () => Temporal.PlainMonthDay.from(fields),
            () => Temporal.PlainYearMonth.from(fields),
            () => Temporal.ZonedDateTime.from(fields),
        ]) {
            try {
                action();
            } catch (error) {
                if (error instanceof RangeError) failures += 1;
            }
        }
        failures
        "#,
    )?;
    ensure_value(&value, &Value::Number(5.0))
}

#[test]
fn distinguishes_rounded_string_offsets_from_exact_property_bag_offsets() -> TestResult {
    let value = eval(
        r#"
        const days = new Temporal.Duration(0, 0, 0, 31);
        const month = new Temporal.Duration(0, 1);
        const roundedString = "1970-01-01T00:00:00-00:45[Africa/Monrovia]";
        const roundedBag = {
            year: 1970,
            month: 1,
            day: 1,
            offset: "-00:45",
            timeZone: "Africa/Monrovia",
        };
        let rejected = false;
        try {
            Temporal.Duration.compare(days, month, { relativeTo: roundedBag });
        } catch (error) {
            rejected = error instanceof RangeError;
        }
        Temporal.Duration.compare(days, month, { relativeTo: roundedString }) === 0 && rejected
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn temporal_with_rejects_temporal_objects_and_empty_bags() -> TestResult {
    let value = eval(
        r"
        const date = new Temporal.PlainDate(2026, 7, 13);
        const dateTime = new Temporal.PlainDateTime(2026, 7, 13);
        let rejected = 0;
        for (const action of [
            () => date.with(date),
            () => date.with({}),
            () => dateTime.with(dateTime),
            () => dateTime.with({}),
        ]) {
            try {
                action();
            } catch (error) {
                if (error instanceof TypeError) rejected += 1;
            }
        }
        rejected
        ",
    )?;
    ensure_value(&value, &Value::Number(4.0))
}
