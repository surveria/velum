use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use num_traits::ToPrimitive;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        object::{DateTimeFormatValue, TemporalValue},
    },
    value::{ErrorName, Value},
};
use temporal_rs::{Calendar, Instant, TimeZone};

use super::date_time_text::{
    flexible_day_period, format_month, localize_numeric_parts, time_zone_name, weekday_name,
    year_parts,
};
use super::date_time_types::{DateTimeInput, DateTimeInputKind, FormatPart};

impl Context {
    pub(super) fn eval_intl_date_time_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        to_parts: bool,
    ) -> Result<Value> {
        let formatter = self.date_time_format_receiver(this_value)?;
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let input = self.intl_date_time_input(&formatter, &value)?;
        if input.kind == DateTimeInputKind::ZonedDateTime {
            return Err(Error::type_error(
                "Temporal.ZonedDateTime is not supported by DateTimeFormat methods",
            ));
        }
        let parts = format_parts(&formatter, &input)?;
        if to_parts {
            return self.intl_parts_value(parts);
        }
        let text = parts.into_iter().map(|part| part.value).collect::<String>();
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn format_temporal_locale_string(
        &mut self,
        value: &Value,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        if let Value::Object(id) = value
            && matches!(
                self.objects.temporal_value(*id)?,
                Some(TemporalValue::ZonedDateTime(_))
            )
            && let Some(options) = args.as_slice().get(1)
            && !matches!(options, Value::Undefined)
            && !matches!(self.get_named(options, "timeZone")?, Value::Undefined)
        {
            return Err(Error::type_error(
                "timeZone option is invalid for Temporal.ZonedDateTime",
            ));
        }
        let formatter = self.parse_date_time_format(args)?;
        let input = self.intl_date_time_input(&formatter, value)?;
        let parts = format_parts(&formatter, &input)?;
        let text = parts.into_iter().map(|part| part.value).collect::<String>();
        self.heap_string_value(&text)
    }

    pub(super) fn intl_date_time_input(
        &mut self,
        formatter: &DateTimeFormatValue,
        value: &Value,
    ) -> Result<DateTimeInput> {
        if let Value::Object(id) = value {
            if let Some(temporal) = self.objects.temporal_value(*id)? {
                return temporal_input(formatter, temporal.clone());
            }
            if let Some(date) = self.objects.date_value(*id)? {
                let millis = date
                    .millis()
                    .ok_or_else(|| Error::exception(ErrorName::RangeError, "Invalid Date"))?;
                return legacy_millis_input(formatter, i128::from(millis));
            }
        }
        if matches!(value, Value::Undefined) {
            return legacy_millis_input(formatter, current_time_millis()?);
        }
        let number = self.to_number(value)?;
        date_time_number_input(formatter, number)
    }

    fn intl_parts_value(&mut self, parts: Vec<FormatPart>) -> Result<Value> {
        let mut values = Vec::with_capacity(parts.len());
        for part in parts {
            let kind = self.heap_string_value(part.kind)?;
            let value = self.heap_string_value(&part.value)?;
            values.push(self.create_intl_data_object(vec![("type", kind), ("value", value)])?);
        }
        self.create_array_from_elements(values)
    }
}

pub(super) fn date_time_number_input(
    formatter: &DateTimeFormatValue,
    number: f64,
) -> Result<DateTimeInput> {
    if !number.is_finite() || number.abs() > 8_640_000_000_000_000.0 {
        return Err(Error::exception(
            ErrorName::RangeError,
            "Date-time value is outside the valid range",
        ));
    }
    let millis = number
        .trunc()
        .to_i128()
        .ok_or_else(|| Error::limit("Date-time milliseconds exceeded i128"))?;
    legacy_millis_input(formatter, millis)
}

fn temporal_input(formatter: &DateTimeFormatValue, value: TemporalValue) -> Result<DateTimeInput> {
    let calendar = Calendar::from_str(&formatter.calendar).map_err(intl_temporal_error)?;
    match value {
        TemporalValue::Duration(_) => {
            Err(Error::type_error("Duration cannot be date-time formatted"))
        }
        TemporalValue::Instant(instant) => {
            instant_input(formatter, instant, DateTimeInputKind::Instant)
        }
        TemporalValue::PlainDate(date) => plain_date_input(date, calendar),
        TemporalValue::PlainDateTime(date_time) => plain_date_time_input(date_time, calendar),
        TemporalValue::PlainMonthDay(month_day) => plain_month_day_input(&month_day, &calendar),
        TemporalValue::PlainTime(time) => Ok(plain_time_input(time)),
        TemporalValue::PlainYearMonth(year_month) => plain_year_month_input(&year_month, &calendar),
        TemporalValue::ZonedDateTime(zoned) => zoned_date_time_input(zoned, calendar),
    }
}

fn plain_date_input(date: temporal_rs::PlainDate, calendar: Calendar) -> Result<DateTimeInput> {
    check_calendar(date.calendar(), &calendar)?;
    let date = if date.calendar().is_iso() {
        date.with_calendar(calendar)
    } else {
        date
    };
    let month_code = date.month_code();
    Ok(DateTimeInput {
        kind: DateTimeInputKind::PlainDate,
        year: Some(date.era_year().unwrap_or_else(|| date.year())),
        era: date.era().map(|era| era.to_string()),
        month: Some(month_code.to_month_integer()),
        month_code: Some(month_code.as_str().to_owned()),
        day: Some(date.day()),
        weekday: Some(date.day_of_week()),
        hour: None,
        minute: None,
        second: None,
        millisecond: None,
        time_zone: None,
        offset: None,
    })
}

fn plain_date_time_input(
    date_time: temporal_rs::PlainDateTime,
    calendar: Calendar,
) -> Result<DateTimeInput> {
    check_calendar(date_time.calendar(), &calendar)?;
    let date_time = if date_time.calendar().is_iso() {
        date_time.with_calendar(calendar)
    } else {
        date_time
    };
    let month_code = date_time.month_code();
    Ok(DateTimeInput {
        kind: DateTimeInputKind::PlainDateTime,
        year: Some(date_time.era_year().unwrap_or_else(|| date_time.year())),
        era: date_time.era().map(|era| era.to_string()),
        month: Some(month_code.to_month_integer()),
        month_code: Some(month_code.as_str().to_owned()),
        day: Some(date_time.day()),
        weekday: Some(date_time.day_of_week()),
        hour: Some(date_time.hour()),
        minute: Some(date_time.minute()),
        second: Some(date_time.second()),
        millisecond: Some(date_time.millisecond()),
        time_zone: None,
        offset: None,
    })
}

fn plain_month_day_input(
    month_day: &temporal_rs::PlainMonthDay,
    calendar: &Calendar,
) -> Result<DateTimeInput> {
    check_calendar_exact(month_day.calendar(), calendar)?;
    Ok(DateTimeInput {
        kind: DateTimeInputKind::PlainMonthDay,
        year: Some(month_day.reference_year()),
        era: None,
        month: Some(month_day.month_code().to_month_integer()),
        month_code: Some(month_day.month_code().as_str().to_owned()),
        day: Some(month_day.day()),
        weekday: None,
        hour: None,
        minute: None,
        second: None,
        millisecond: None,
        time_zone: None,
        offset: None,
    })
}

const fn plain_time_input(time: temporal_rs::PlainTime) -> DateTimeInput {
    DateTimeInput {
        kind: DateTimeInputKind::PlainTime,
        year: None,
        era: None,
        month: None,
        month_code: None,
        day: None,
        weekday: None,
        hour: Some(time.hour()),
        minute: Some(time.minute()),
        second: Some(time.second()),
        millisecond: Some(time.millisecond()),
        time_zone: None,
        offset: None,
    }
}

fn plain_year_month_input(
    year_month: &temporal_rs::PlainYearMonth,
    calendar: &Calendar,
) -> Result<DateTimeInput> {
    check_calendar_exact(year_month.calendar(), calendar)?;
    let month_code = year_month.month_code();
    Ok(DateTimeInput {
        kind: DateTimeInputKind::PlainYearMonth,
        year: Some(year_month.era_year().unwrap_or_else(|| year_month.year())),
        era: year_month.era().map(|era| era.to_string()),
        month: Some(month_code.to_month_integer()),
        month_code: Some(month_code.as_str().to_owned()),
        day: Some(year_month.reference_day()),
        weekday: None,
        hour: None,
        minute: None,
        second: None,
        millisecond: None,
        time_zone: None,
        offset: None,
    })
}

fn zoned_date_time_input(
    zoned: temporal_rs::ZonedDateTime,
    calendar: Calendar,
) -> Result<DateTimeInput> {
    check_calendar(zoned.calendar(), &calendar)?;
    let zoned = if zoned.calendar().is_iso() {
        zoned.with_calendar(calendar)
    } else {
        zoned
    };
    let time_zone = zoned
        .time_zone()
        .identifier()
        .map_err(intl_temporal_error)?;
    let month_code = zoned.month_code();
    Ok(DateTimeInput {
        kind: DateTimeInputKind::ZonedDateTime,
        year: Some(zoned.era_year().unwrap_or_else(|| zoned.year())),
        era: zoned.era().map(|era| era.to_string()),
        month: Some(month_code.to_month_integer()),
        month_code: Some(month_code.as_str().to_owned()),
        day: Some(zoned.day()),
        weekday: Some(zoned.day_of_week()),
        hour: Some(zoned.hour()),
        minute: Some(zoned.minute()),
        second: Some(zoned.second()),
        millisecond: Some(zoned.millisecond()),
        time_zone: Some(time_zone),
        offset: Some(zoned.offset()),
    })
}

fn instant_input(
    formatter: &DateTimeFormatValue,
    instant: Instant,
    kind: DateTimeInputKind,
) -> Result<DateTimeInput> {
    let time_zone =
        TimeZone::try_from_identifier_str(&formatter.time_zone).map_err(intl_temporal_error)?;
    let calendar = Calendar::from_str(&formatter.calendar).map_err(intl_temporal_error)?;
    let zoned = instant
        .to_zoned_date_time_iso(time_zone)
        .map_err(intl_temporal_error)?
        .with_calendar(calendar);
    let time_zone = zoned
        .time_zone()
        .identifier()
        .map_err(intl_temporal_error)?;
    let month_code = zoned.month_code();
    Ok(DateTimeInput {
        kind,
        year: Some(zoned.era_year().unwrap_or_else(|| zoned.year())),
        era: zoned.era().map(|era| era.to_string()),
        month: Some(month_code.to_month_integer()),
        month_code: Some(month_code.as_str().to_owned()),
        day: Some(zoned.day()),
        weekday: Some(zoned.day_of_week()),
        hour: Some(zoned.hour()),
        minute: Some(zoned.minute()),
        second: Some(zoned.second()),
        millisecond: Some(zoned.millisecond()),
        time_zone: Some(time_zone),
        offset: Some(zoned.offset()),
    })
}

pub(super) fn format_parts(
    formatter: &DateTimeFormatValue,
    input: &DateTimeInput,
) -> Result<Vec<FormatPart>> {
    validate_styles(formatter, input.kind)?;
    let (show_date, show_time, show_zone) = selected_groups(formatter, input.kind);
    let mut parts = Vec::new();
    if show_date {
        append_date_parts(&mut parts, formatter, input)?;
    }
    if show_date && show_time {
        parts.push(literal(", "));
    }
    if show_time {
        append_time_parts(&mut parts, formatter, input)?;
    }
    if show_zone {
        if show_date || show_time {
            parts.push(literal(" "));
        }
        let zone_style = formatter.options.time_zone_name.as_deref().or({
            match formatter.options.time_style.as_deref() {
                Some("full") => Some("long"),
                Some("long") => Some("short"),
                _ => None,
            }
        });
        parts.push(FormatPart {
            kind: "timeZoneName",
            value: time_zone_name(input, zone_style),
        });
    }
    if parts.is_empty() {
        return Err(Error::type_error(
            "DateTimeFormat options do not overlap the input type",
        ));
    }
    localize_numeric_parts(&mut parts, formatter);
    Ok(parts)
}

fn legacy_millis_input(formatter: &DateTimeFormatValue, millis: i128) -> Result<DateTimeInput> {
    let nanos = millis
        .checked_mul(1_000_000)
        .ok_or_else(|| Error::limit("Date nanoseconds overflowed"))?;
    let instant = Instant::try_new(nanos).map_err(intl_temporal_error)?;
    instant_input(formatter, instant, DateTimeInputKind::LegacyDate)
}

fn current_time_millis() -> Result<i128> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| Error::runtime(error.to_string()))?;
    i128::try_from(duration.as_millis())
        .map_err(|error| Error::limit(format!("current time milliseconds overflowed: {error}")))
}

fn check_calendar(actual: &Calendar, requested: &Calendar) -> Result<()> {
    if actual.is_iso() || actual.identifier() == requested.identifier() {
        return Ok(());
    }
    Err(Error::exception(
        ErrorName::RangeError,
        "Temporal calendar does not match Intl calendar",
    ))
}

fn check_calendar_exact(actual: &Calendar, requested: &Calendar) -> Result<()> {
    if actual.identifier() == requested.identifier() {
        return Ok(());
    }
    Err(Error::exception(
        ErrorName::RangeError,
        "Temporal calendar does not match Intl calendar",
    ))
}

fn validate_styles(formatter: &DateTimeFormatValue, kind: DateTimeInputKind) -> Result<()> {
    let date_only = matches!(
        kind,
        DateTimeInputKind::PlainDate
            | DateTimeInputKind::PlainMonthDay
            | DateTimeInputKind::PlainYearMonth
    );
    if date_only && formatter.options.time_style.is_some() && formatter.options.date_style.is_none()
    {
        return Err(Error::type_error(
            "timeStyle is invalid for this Temporal value",
        ));
    }
    if kind == DateTimeInputKind::PlainTime
        && formatter.options.date_style.is_some()
        && formatter.options.time_style.is_none()
    {
        return Err(Error::type_error(
            "dateStyle is invalid for Temporal.PlainTime",
        ));
    }
    Ok(())
}

fn selected_groups(formatter: &DateTimeFormatValue, kind: DateTimeInputKind) -> (bool, bool, bool) {
    let explicit_date =
        !formatter.options.default_components && formatter.options.has_explicit_date_fields();
    let explicit_time =
        !formatter.options.default_components && formatter.options.has_explicit_time_fields();
    let only_era = formatter.options.era.is_some()
        && formatter.options.weekday.is_none()
        && formatter.options.year.is_none()
        && formatter.options.month.is_none()
        && formatter.options.day.is_none()
        && !explicit_time;
    let only_zone = formatter.options.time_zone_name.is_some()
        && formatter.options.hour.is_none()
        && formatter.options.minute.is_none()
        && formatter.options.second.is_none()
        && formatter.options.day_period.is_none()
        && formatter.options.fractional_second_digits.is_none()
        && !explicit_date;
    let no_components = formatter.options.default_components;
    let default_date = matches!(
        kind,
        DateTimeInputKind::Instant
            | DateTimeInputKind::PlainDate
            | DateTimeInputKind::PlainDateTime
            | DateTimeInputKind::PlainMonthDay
            | DateTimeInputKind::PlainYearMonth
            | DateTimeInputKind::ZonedDateTime
            | DateTimeInputKind::LegacyDate
    );
    let default_time = matches!(
        kind,
        DateTimeInputKind::Instant
            | DateTimeInputKind::PlainDateTime
            | DateTimeInputKind::PlainTime
            | DateTimeInputKind::ZonedDateTime
    );
    let date_capable = kind != DateTimeInputKind::PlainTime;
    let time_capable = matches!(
        kind,
        DateTimeInputKind::Instant
            | DateTimeInputKind::PlainDateTime
            | DateTimeInputKind::PlainTime
            | DateTimeInputKind::ZonedDateTime
            | DateTimeInputKind::LegacyDate
    );
    let show_date = date_capable
        && (formatter.options.date_style.is_some()
            || (explicit_date && !only_era)
            || ((no_components || only_era || only_zone) && default_date));
    let era_default_time = only_era
        && matches!(
            kind,
            DateTimeInputKind::Instant
                | DateTimeInputKind::PlainDateTime
                | DateTimeInputKind::PlainTime
        );
    let show_time = time_capable
        && (formatter.options.time_style.is_some()
            || explicit_time
            || (no_components && default_time)
            || era_default_time
            || (only_zone && default_time));
    let zone_capable = matches!(
        kind,
        DateTimeInputKind::Instant
            | DateTimeInputKind::ZonedDateTime
            | DateTimeInputKind::LegacyDate
    );
    let style_zone = matches!(
        formatter.options.time_style.as_deref(),
        Some("full" | "long")
    );
    let show_zone = zone_capable
        && (formatter.options.time_zone_name.is_some()
            || style_zone
            || (no_components && kind == DateTimeInputKind::ZonedDateTime));
    (show_date, show_time, show_zone)
}

fn append_date_parts(
    parts: &mut Vec<FormatPart>,
    formatter: &DateTimeFormatValue,
    input: &DateTimeInput,
) -> Result<()> {
    let year = input
        .year
        .ok_or_else(|| Error::type_error("Date year is unavailable"))?;
    let month = input
        .month
        .ok_or_else(|| Error::type_error("Date month is unavailable"))?;
    let day = input
        .day
        .ok_or_else(|| Error::type_error("Date day is unavailable"))?;
    let options = &formatter.options;
    let default_components = options.default_components
        || options.date_style.is_some()
        || options.era.is_some()
        || (options.time_zone_name.is_some() && !options.has_explicit_date_fields());
    let supports_year = input.kind != DateTimeInputKind::PlainMonthDay;
    let supports_day = input.kind != DateTimeInputKind::PlainYearMonth;
    let show_year = supports_year && (options.year.is_some() || default_components);
    let show_month = options.month.is_some() || default_components;
    let show_day = supports_day && (options.day.is_some() || default_components);
    let show_weekday = input.weekday.is_some()
        && (options.weekday.is_some() || options.date_style.as_deref() == Some("full"));
    if show_weekday {
        let weekday = weekday_name(input.weekday.unwrap_or(1), &formatter.locale);
        parts.push(FormatPart {
            kind: "weekday",
            value: weekday.to_owned(),
        });
        if show_year || show_month || show_day {
            parts.push(literal(", "));
        }
    }
    let month_style = options
        .month
        .as_deref()
        .or(match options.date_style.as_deref() {
            Some("full" | "long") => Some("long"),
            Some("medium") => Some("short"),
            _ => None,
        });
    let year_style = options
        .year
        .as_deref()
        .or_else(|| (options.date_style.as_deref() == Some("short")).then_some("2-digit"));
    let month_text = format_month(
        month,
        input.month_code.as_deref(),
        month_style,
        &formatter.calendar,
    );
    let german = formatter.locale.to_ascii_lowercase().starts_with("de");
    let japanese = formatter.locale.to_ascii_lowercase().starts_with("ja");
    append_date_layout(
        parts,
        formatter,
        DatePattern {
            year: show_year.then_some((year, year_style)),
            month_text: show_month.then_some(month_text),
            day: show_day.then_some(day),
            layout: if japanese {
                DateLayout::Japanese
            } else if german {
                DateLayout::German
            } else if matches!(month_style, Some("long" | "short" | "narrow")) {
                DateLayout::Textual
            } else {
                DateLayout::Numeric
            },
        },
    );
    if options.era.is_some()
        && let Some(era) = input.era.as_deref()
    {
        parts.push(literal(" "));
        parts.push(FormatPart {
            kind: "era",
            value: match era {
                "ce" => "AD",
                "bce" => "BC",
                other => other,
            }
            .to_owned(),
        });
    }
    Ok(())
}

struct DatePattern<'a> {
    year: Option<(i32, Option<&'a str>)>,
    month_text: Option<String>,
    day: Option<u8>,
    layout: DateLayout,
}

enum DateLayout {
    Japanese,
    German,
    Textual,
    Numeric,
}

fn append_date_layout(
    parts: &mut Vec<FormatPart>,
    formatter: &DateTimeFormatValue,
    pattern: DatePattern<'_>,
) {
    let show_year = pattern.year.is_some();
    let show_month = pattern.month_text.is_some();
    let show_day = pattern.day.is_some();
    let pattern_year = pattern.year.map_or_else(Vec::new, |(year, style)| {
        year_parts(year, style, &formatter.calendar, &formatter.locale)
    });
    let day_text = pattern.day.map(|day| day.to_string());
    match pattern.layout {
        DateLayout::Japanese => {
            parts.extend(pattern_year);
            append_literal_if(parts, show_year && show_month, "/");
            append_optional(parts, "month", pattern.month_text);
            append_literal_if(parts, show_month && show_day, "/");
            append_optional(parts, "day", day_text);
        }
        DateLayout::German => {
            append_optional(parts, "day", day_text);
            append_literal_if(parts, show_day && show_month, ".");
            append_optional(parts, "month", pattern.month_text);
            append_literal_if(parts, show_month && show_year, ".");
            parts.extend(pattern_year);
        }
        DateLayout::Textual => {
            append_optional(parts, "month", pattern.month_text);
            append_literal_if(parts, show_month && show_day, " ");
            append_optional(parts, "day", day_text);
            append_literal_if(parts, (show_month || show_day) && show_year, ", ");
            parts.extend(pattern_year);
        }
        DateLayout::Numeric => {
            append_optional(parts, "month", pattern.month_text);
            append_literal_if(parts, show_month && show_day, "/");
            append_optional(parts, "day", day_text);
            append_literal_if(parts, (show_month || show_day) && show_year, "/");
            parts.extend(pattern_year);
        }
    }
}

fn append_optional(parts: &mut Vec<FormatPart>, kind: &'static str, value: Option<String>) {
    if let Some(value) = value {
        parts.push(FormatPart { kind, value });
    }
}

fn append_time_parts(
    parts: &mut Vec<FormatPart>,
    formatter: &DateTimeFormatValue,
    input: &DateTimeInput,
) -> Result<()> {
    let options = &formatter.options;
    let style = options.time_style.as_deref();
    let only_era = options.era.is_some()
        && options.weekday.is_none()
        && options.year.is_none()
        && options.month.is_none()
        && options.day.is_none()
        && options.hour.is_none()
        && options.minute.is_none()
        && options.second.is_none()
        && options.day_period.is_none()
        && options.fractional_second_digits.is_none();
    let only_zone = options.time_zone_name.is_some()
        && options.hour.is_none()
        && options.minute.is_none()
        && options.second.is_none()
        && options.day_period.is_none()
        && options.fractional_second_digits.is_none()
        && !options.has_explicit_date_fields();
    let no_components = options.default_components || only_era || only_zone;
    let show_hour = options.hour.is_some() || no_components || style.is_some();
    let show_minute = options.minute.is_some() || no_components || style.is_some();
    let show_second = options.second.is_some()
        || no_components
        || matches!(style, Some("full" | "long" | "medium"));
    let hour = input
        .hour
        .ok_or_else(|| Error::type_error("Hour is unavailable"))?;
    let minute = input.minute.unwrap_or(0);
    let second = input.second.unwrap_or(0);
    let cycle = resolved_cycle(formatter);
    let displayed_hour = match cycle {
        "h11" => hour % 12,
        "h12" => {
            if hour % 12 == 0 {
                12
            } else {
                hour % 12
            }
        }
        "h24" => {
            if hour == 0 {
                24
            } else {
                hour
            }
        }
        _ => hour,
    };
    let pad_hour = matches!(cycle, "h23" | "h24") || options.hour.as_deref() == Some("2-digit");
    append_selected(
        parts,
        show_hour,
        "hour",
        if pad_hour {
            format!("{displayed_hour:02}")
        } else {
            displayed_hour.to_string()
        },
    );
    append_literal_if(parts, show_hour && show_minute, ":");
    append_selected(parts, show_minute, "minute", format!("{minute:02}"));
    append_literal_if(parts, (show_hour || show_minute) && show_second, ":");
    append_selected(parts, show_second, "second", format!("{second:02}"));
    if let Some(digits) = options.fractional_second_digits {
        let millis = input.millisecond.unwrap_or(0);
        let text = format!("{millis:03}");
        let end = usize::from(digits);
        let fraction = text.get(..end).unwrap_or(text.as_str()).to_owned();
        if show_second {
            parts.push(literal("."));
        }
        parts.push(FormatPart {
            kind: "fractionalSecond",
            value: fraction,
        });
    }
    if options.day_period.is_some() || (show_hour && matches!(cycle, "h11" | "h12")) {
        if show_hour || show_minute || show_second {
            parts.push(literal(" "));
        }
        parts.push(FormatPart {
            kind: "dayPeriod",
            value: options.day_period.as_deref().map_or_else(
                || {
                    if hour < 12 {
                        "AM".to_owned()
                    } else {
                        "PM".to_owned()
                    }
                },
                |day_period| flexible_day_period(hour, Some(day_period)).to_owned(),
            ),
        });
    }
    Ok(())
}

fn append_selected(parts: &mut Vec<FormatPart>, selected: bool, kind: &'static str, value: String) {
    if selected {
        parts.push(FormatPart { kind, value });
    }
}

fn append_literal_if(parts: &mut Vec<FormatPart>, selected: bool, text: &'static str) {
    if selected {
        parts.push(literal(text));
    }
}

fn literal(value: &'static str) -> FormatPart {
    FormatPart {
        kind: "literal",
        value: value.to_owned(),
    }
}

fn resolved_cycle(formatter: &DateTimeFormatValue) -> &str {
    formatter.options.hour_cycle.as_deref().unwrap_or("h12")
}

fn intl_temporal_error(error: temporal_rs::TemporalError) -> Error {
    Error::exception(ErrorName::RangeError, error.to_string())
}
