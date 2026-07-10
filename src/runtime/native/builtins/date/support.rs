use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    error::{Error, Result},
    runtime::{Context, abstract_operations::integer_or_infinity_from_number, object::DateValue},
    value::Value,
};

const INVALID_DATE: &str = "Invalid Date";
const ISO_DATE_TIME_SEPARATOR: char = 'T';
const ISO_UTC_SUFFIX: char = 'Z';
const MAX_TIME_MS: i64 = 8_640_000_000_000_000;
const MAX_TIME_MS_NUMBER: f64 = 8_640_000_000_000_000.0;
const MAX_COMPONENT_ABS: i64 = 1_000_000;
const MS_PER_DAY: i64 = 86_400_000;
const MS_PER_HOUR: i64 = 3_600_000;
const MS_PER_MINUTE: i64 = 60_000;
const MS_PER_SECOND: i64 = 1_000;
const MONTHS_PER_YEAR: i64 = 12;
const YEAR_OFFSET_1900: i64 = 1_900;
const SHORT_MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const SHORT_WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

#[derive(Debug, Clone, Copy)]
pub(super) enum DateComponent {
    FullYear,
    Month,
    Date,
    Day,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct DateParts {
    pub(super) year: i64,
    pub(super) month: i64,
    pub(super) date: i64,
    pub(super) day: i64,
    pub(super) hour: i64,
    pub(super) minute: i64,
    pub(super) second: i64,
    pub(super) millisecond: i64,
}

impl DateParts {
    pub(super) fn from_millis(ms: i64) -> Result<Self> {
        let day = ms.div_euclid(MS_PER_DAY);
        let time = ms.rem_euclid(MS_PER_DAY);
        let (year, month, date) = civil_from_days(day);
        Ok(Self {
            year,
            month: month
                .checked_sub(1)
                .ok_or_else(|| Error::runtime("date month underflowed"))?,
            date,
            day: day
                .checked_add(4)
                .ok_or_else(|| Error::runtime("date weekday overflowed"))?
                .rem_euclid(7),
            hour: time / MS_PER_HOUR,
            minute: (time % MS_PER_HOUR) / MS_PER_MINUTE,
            second: (time % MS_PER_MINUTE) / MS_PER_SECOND,
            millisecond: time % MS_PER_SECOND,
        })
    }
}

pub(super) fn component_value(parts: DateParts, component: DateComponent) -> Result<f64> {
    let value = match component {
        DateComponent::FullYear => parts.year,
        DateComponent::Month => parts.month,
        DateComponent::Date => parts.date,
        DateComponent::Day => parts.day,
        DateComponent::Hours => parts.hour,
        DateComponent::Minutes => parts.minute,
        DateComponent::Seconds => parts.second,
        DateComponent::Milliseconds => parts.millisecond,
    };
    integer_to_number(value)
}

pub(super) fn current_time_value() -> Result<DateValue> {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration_to_date_value(duration),
        Err(error) => {
            let millis = duration_to_millis(error.duration())?;
            let value = millis
                .checked_neg()
                .ok_or_else(|| Error::limit("system time value overflowed"))?;
            Ok(DateValue::from_millis(value))
        }
    }
}

fn duration_to_date_value(duration: Duration) -> Result<DateValue> {
    duration_to_millis(duration).map(DateValue::from_millis)
}

fn duration_to_millis(duration: Duration) -> Result<i64> {
    i64::try_from(duration.as_millis()).map_err(|_| Error::limit("system time value overflowed"))
}

pub(super) fn date_value_to_number(value: DateValue) -> Result<f64> {
    match value {
        DateValue::Invalid => Ok(f64::NAN),
        DateValue::Milliseconds(value) => integer_to_number(value),
    }
}

pub(super) fn time_clip(value: f64) -> Result<DateValue> {
    if !value.is_finite() || value.abs() > MAX_TIME_MS_NUMBER {
        return Ok(DateValue::Invalid);
    }
    date_integer_from_number(value).map(DateValue::from_millis)
}

fn date_integer_from_number(value: f64) -> Result<i64> {
    let integer = integer_or_infinity_from_number(value);
    format!("{integer:.0}")
        .parse::<i64>()
        .map_err(|error| Error::runtime(format!("failed to convert Date number: {error}")))
}

pub(super) fn integer_to_number(value: i64) -> Result<f64> {
    value
        .to_string()
        .parse::<f64>()
        .map_err(|error| Error::runtime(format!("failed to convert Date integer: {error}")))
}

pub(super) fn integer_component(
    context: &mut Context,
    value: Option<&Value>,
) -> Result<Option<i64>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let number = context.to_number(value)?;
    if !number.is_finite() || number.abs() > MAX_TIME_MS_NUMBER {
        return Ok(None);
    }
    let integer = date_integer_from_number(number)?;
    if integer.abs() > MAX_COMPONENT_ABS {
        return Ok(None);
    }
    Ok(Some(integer))
}

pub(super) fn integer_component_with_default(
    context: &mut Context,
    value: Option<&Value>,
    default: i64,
) -> Result<Option<i64>> {
    if value.is_none() {
        return Ok(Some(default));
    }
    integer_component(context, value)
}

pub(super) fn normalize_component_year(year: i64) -> i64 {
    if (0..=99).contains(&year) {
        return year.saturating_add(YEAR_OFFSET_1900);
    }
    year
}

pub(super) fn make_date_value(
    year: i64,
    month: i64,
    date: i64,
    hour: i64,
    minute: i64,
    second: i64,
    millisecond: i64,
) -> DateValue {
    let Some(day) = make_day(year, month, date) else {
        return DateValue::Invalid;
    };
    let Some(time) = make_time(hour, minute, second, millisecond) else {
        return DateValue::Invalid;
    };
    let Some(value) = day
        .checked_mul(MS_PER_DAY)
        .and_then(|day_ms| day_ms.checked_add(time))
    else {
        return DateValue::Invalid;
    };
    if value.abs() > MAX_TIME_MS {
        return DateValue::Invalid;
    }
    DateValue::from_millis(value)
}

fn make_day(year: i64, month: i64, date: i64) -> Option<i64> {
    let year = year.checked_add(month.div_euclid(MONTHS_PER_YEAR))?;
    let month = month.rem_euclid(MONTHS_PER_YEAR).checked_add(1)?;
    let first = days_from_civil(year, month, 1)?;
    first.checked_add(date.checked_sub(1)?)
}

fn make_time(hour: i64, minute: i64, second: i64, millisecond: i64) -> Option<i64> {
    hour.checked_mul(MS_PER_HOUR)?
        .checked_add(minute.checked_mul(MS_PER_MINUTE)?)?
        .checked_add(second.checked_mul(MS_PER_SECOND)?)?
        .checked_add(millisecond)
}

pub(super) fn parse_date_string(text: &str) -> Result<DateValue> {
    let text = text.trim();
    let (date, time) = match text.split_once(ISO_DATE_TIME_SEPARATOR) {
        Some((date, time)) => (date, Some(time)),
        None => (text, None),
    };
    let Some((year, month, day)) = parse_date_part(date) else {
        return Ok(DateValue::Invalid);
    };
    let Some((hour, minute, second, millisecond)) = parse_time_part(time)? else {
        return Ok(DateValue::Invalid);
    };
    Ok(make_date_value(
        year,
        month
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("parsed Date month underflowed"))?,
        day,
        hour,
        minute,
        second,
        millisecond,
    ))
}

fn parse_date_part(text: &str) -> Option<(i64, i64, i64)> {
    let mut parts = text.split('-');
    let year = parse_fixed_digits(parts.next()?, 4)?;
    let month = parse_fixed_digits(parts.next()?, 2)?;
    let day = parse_fixed_digits(parts.next()?, 2)?;
    if parts.next().is_some() || !is_valid_month_day(year, month, day) {
        return None;
    }
    Some((year, month, day))
}

fn parse_time_part(text: Option<&str>) -> Result<Option<(i64, i64, i64, i64)>> {
    let Some(text) = text else {
        return Ok(Some((0, 0, 0, 0)));
    };
    let text = text.strip_suffix(ISO_UTC_SUFFIX).unwrap_or(text);
    let mut parts = text.split(':');
    let Some(hour_text) = parts.next() else {
        return Ok(None);
    };
    let Some(minute_text) = parts.next() else {
        return Ok(None);
    };
    let Some(second_text) = parts.next() else {
        return Ok(None);
    };
    if parts.next().is_some() {
        return Ok(None);
    }
    let hour = parse_fixed_digits(hour_text, 2);
    let minute = parse_fixed_digits(minute_text, 2);
    let (second, millisecond) = parse_second_and_millisecond(second_text)?;
    let (Some(hour), Some(minute), Some(second)) = (hour, minute, second) else {
        return Ok(None);
    };
    if hour > 23 || minute > 59 || second > 59 {
        return Ok(None);
    }
    Ok(Some((hour, minute, second, millisecond)))
}

fn parse_second_and_millisecond(text: &str) -> Result<(Option<i64>, i64)> {
    let Some((second, fraction)) = text.split_once('.') else {
        return Ok((parse_fixed_digits(text, 2), 0));
    };
    let Some(second) = parse_fixed_digits(second, 2) else {
        return Ok((None, 0));
    };
    parse_millisecond_fraction(fraction).map(|millisecond| (Some(second), millisecond))
}

fn parse_millisecond_fraction(text: &str) -> Result<i64> {
    let mut digits = String::with_capacity(3);
    for ch in text.chars().take(3) {
        if !ch.is_ascii_digit() {
            return Ok(0);
        }
        digits.push(ch);
    }
    while digits.len() < 3 {
        digits.push('0');
    }
    digits
        .parse::<i64>()
        .map_err(|error| Error::runtime(format!("failed to parse millisecond fraction: {error}")))
}

fn parse_fixed_digits(text: &str, count: usize) -> Option<i64> {
    if text.len() != count || !text.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

fn is_valid_month_day(year: i64, month: i64, day: i64) -> bool {
    if !(1..=12).contains(&month) {
        return false;
    }
    (1..=days_in_month(year, month)).contains(&day)
}

const fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

const fn is_leap_year(year: i64) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    let shifted_year = if month <= 2 {
        year.checked_sub(1)?
    } else {
        year
    };
    let era = shifted_year.div_euclid(400);
    let yoe = shifted_year.checked_sub(era.checked_mul(400)?)?;
    let mp = month.checked_add(if month > 2 { -3 } else { 9 })?;
    let doy = (153_i64)
        .checked_mul(mp)?
        .checked_add(2)?
        .checked_div(5)?
        .checked_add(day)?
        .checked_sub(1)?;
    let doe = yoe
        .checked_mul(365)?
        .checked_add(yoe / 4)?
        .checked_sub(yoe / 100)?
        .checked_add(doy)?;
    era.checked_mul(146_097)?
        .checked_add(doe)?
        .checked_sub(719_468)
}

const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

pub(super) fn format_iso_string(ms: i64) -> Result<String> {
    let parts = DateParts::from_millis(ms)?;
    let year = format_iso_year(parts.year);
    Ok(format!(
        "{year}-{month:02}-{date:02}T{hour:02}:{minute:02}:{second:02}.{millisecond:03}Z",
        month = parts.month + 1,
        date = parts.date,
        hour = parts.hour,
        minute = parts.minute,
        second = parts.second,
        millisecond = parts.millisecond
    ))
}

fn format_iso_year(year: i64) -> String {
    if (0..=9_999).contains(&year) {
        return format!("{year:04}");
    }
    if year >= 0 {
        return format!("+{year:06}");
    }
    format!("-{:06}", year.saturating_abs())
}

pub(super) fn format_date_time_string(value: DateValue) -> Result<String> {
    if value.millis().is_none() {
        return Ok(INVALID_DATE.to_owned());
    }
    let date = format_date_only_string(value)?;
    let time = format_time_only_string(value)?;
    Ok(format!("{date} {time}"))
}

pub(super) fn format_date_only_string(value: DateValue) -> Result<String> {
    let Some(ms) = value.millis() else {
        return Ok(INVALID_DATE.to_owned());
    };
    let parts = DateParts::from_millis(ms)?;
    let weekday = name_from_table(&SHORT_WEEKDAY_NAMES, parts.day)?;
    let month = name_from_table(&SHORT_MONTH_NAMES, parts.month)?;
    Ok(format!(
        "{weekday} {month} {date:02} {year:04}",
        date = parts.date,
        year = parts.year
    ))
}

pub(super) fn format_time_only_string(value: DateValue) -> Result<String> {
    let Some(ms) = value.millis() else {
        return Ok(INVALID_DATE.to_owned());
    };
    let parts = DateParts::from_millis(ms)?;
    Ok(format!(
        "{hour:02}:{minute:02}:{second:02} GMT+0000 (UTC)",
        hour = parts.hour,
        minute = parts.minute,
        second = parts.second
    ))
}

pub(super) fn format_utc_string(value: DateValue) -> Result<String> {
    let Some(ms) = value.millis() else {
        return Ok(INVALID_DATE.to_owned());
    };
    let parts = DateParts::from_millis(ms)?;
    let weekday = name_from_table(&SHORT_WEEKDAY_NAMES, parts.day)?;
    let month = name_from_table(&SHORT_MONTH_NAMES, parts.month)?;
    Ok(format!(
        "{weekday}, {date:02} {month} {year:04} {hour:02}:{minute:02}:{second:02} GMT",
        date = parts.date,
        year = parts.year,
        hour = parts.hour,
        minute = parts.minute,
        second = parts.second
    ))
}

fn name_from_table(table: &[&'static str], index: i64) -> Result<&'static str> {
    let index = usize::try_from(index).map_err(|_| Error::runtime("date name index is invalid"))?;
    table
        .get(index)
        .copied()
        .ok_or_else(|| Error::runtime("date name index is out of range"))
}
