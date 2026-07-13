use std::{cmp::Ordering, str::FromStr};

use num_traits::ToPrimitive;
use temporal_rs::{
    Calendar, MonthCode, PlainDateTime, PlainTime, TimeZone, TinyAsciiStr,
    fields::{CalendarFields, DateTimeFields},
    options::{Disambiguation, DisplayCalendar, Overflow, ToStringRoundingOptions},
    partial::{PartialDateTime, PartialTime},
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::{ErrorName, Value},
};

use super::temporal_error;

enum PlainDateTimeInput {
    Resolved(PlainDateTime),
    String(String),
    Fields(PlainDateTimeFields),
}

struct PlainDateTimeFields {
    calendar: Calendar,
    day: i64,
    era: Option<TinyAsciiStr<19>>,
    era_year: Option<i64>,
    hour: Option<i64>,
    microsecond: Option<i64>,
    millisecond: Option<i64>,
    minute: Option<i64>,
    month: Option<i64>,
    month_code: Option<MonthCode>,
    nanosecond: Option<i64>,
    second: Option<i64>,
    year: Option<i64>,
}

pub(super) const STATIC_METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("from", TemporalFunctionKind::PlainDateTimeFrom),
    ("compare", TemporalFunctionKind::PlainDateTimeCompare),
];

pub(super) const ACCESSORS: &[(&str, TemporalFunctionKind)] = &[
    ("year", TemporalFunctionKind::PlainDateTimePrototypeYear),
    ("month", TemporalFunctionKind::PlainDateTimePrototypeMonth),
    (
        "monthCode",
        TemporalFunctionKind::PlainDateTimePrototypeMonthCode,
    ),
    ("day", TemporalFunctionKind::PlainDateTimePrototypeDay),
    ("hour", TemporalFunctionKind::PlainDateTimePrototypeHour),
    ("minute", TemporalFunctionKind::PlainDateTimePrototypeMinute),
    ("second", TemporalFunctionKind::PlainDateTimePrototypeSecond),
    (
        "millisecond",
        TemporalFunctionKind::PlainDateTimePrototypeMillisecond,
    ),
    (
        "microsecond",
        TemporalFunctionKind::PlainDateTimePrototypeMicrosecond,
    ),
    (
        "nanosecond",
        TemporalFunctionKind::PlainDateTimePrototypeNanosecond,
    ),
    (
        "calendarId",
        TemporalFunctionKind::PlainDateTimePrototypeCalendarId,
    ),
    ("era", TemporalFunctionKind::PlainDateTimePrototypeEra),
    (
        "eraYear",
        TemporalFunctionKind::PlainDateTimePrototypeEraYear,
    ),
    (
        "dayOfWeek",
        TemporalFunctionKind::PlainDateTimePrototypeDayOfWeek,
    ),
    (
        "dayOfYear",
        TemporalFunctionKind::PlainDateTimePrototypeDayOfYear,
    ),
    (
        "weekOfYear",
        TemporalFunctionKind::PlainDateTimePrototypeWeekOfYear,
    ),
    (
        "yearOfWeek",
        TemporalFunctionKind::PlainDateTimePrototypeYearOfWeek,
    ),
    (
        "daysInWeek",
        TemporalFunctionKind::PlainDateTimePrototypeDaysInWeek,
    ),
    (
        "daysInMonth",
        TemporalFunctionKind::PlainDateTimePrototypeDaysInMonth,
    ),
    (
        "daysInYear",
        TemporalFunctionKind::PlainDateTimePrototypeDaysInYear,
    ),
    (
        "monthsInYear",
        TemporalFunctionKind::PlainDateTimePrototypeMonthsInYear,
    ),
    (
        "inLeapYear",
        TemporalFunctionKind::PlainDateTimePrototypeInLeapYear,
    ),
];

pub(super) const METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("with", TemporalFunctionKind::PlainDateTimePrototypeWith),
    (
        "withPlainTime",
        TemporalFunctionKind::PlainDateTimePrototypeWithPlainTime,
    ),
    (
        "withCalendar",
        TemporalFunctionKind::PlainDateTimePrototypeWithCalendar,
    ),
    ("add", TemporalFunctionKind::PlainDateTimePrototypeAdd),
    (
        "subtract",
        TemporalFunctionKind::PlainDateTimePrototypeSubtract,
    ),
    ("until", TemporalFunctionKind::PlainDateTimePrototypeUntil),
    ("since", TemporalFunctionKind::PlainDateTimePrototypeSince),
    ("round", TemporalFunctionKind::PlainDateTimePrototypeRound),
    ("equals", TemporalFunctionKind::PlainDateTimePrototypeEquals),
    (
        "toString",
        TemporalFunctionKind::PlainDateTimePrototypeToString,
    ),
    (
        "toLocaleString",
        TemporalFunctionKind::PlainDateTimePrototypeToLocaleString,
    ),
    ("toJSON", TemporalFunctionKind::PlainDateTimePrototypeToJson),
    (
        "toZonedDateTime",
        TemporalFunctionKind::PlainDateTimePrototypeToZonedDateTime,
    ),
    (
        "toPlainDate",
        TemporalFunctionKind::PlainDateTimePrototypeToPlainDate,
    ),
    (
        "toPlainTime",
        TemporalFunctionKind::PlainDateTimePrototypeToPlainTime,
    ),
    (
        "valueOf",
        TemporalFunctionKind::PlainDateTimePrototypeValueOf,
    ),
];

impl Context {
    pub(super) fn eval_plain_date_time_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if let Some(result) = self.eval_plain_date_time_accessor(kind, receiver) {
            return result;
        }
        match kind {
            TemporalFunctionKind::PlainDateTimeConstructor => Err(Error::type_error(
                "Temporal.PlainDateTime constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainDateTimeFrom => {
                let values = args.as_slice();
                let input = self.prepare_plain_date_time_input(values.first())?;
                if let PlainDateTimeInput::String(text) = input {
                    let date_time =
                        PlainDateTime::from_utf8(text.as_bytes()).map_err(temporal_error)?;
                    self.plain_date_overflow_option(values.get(1))?;
                    return self.create_plain_date_time_value(date_time);
                }
                let overflow = self.plain_date_overflow_option(values.get(1))?;
                let date_time = Self::resolve_plain_date_time_input(input, overflow)?;
                self.create_plain_date_time_value(date_time)
            }
            TemporalFunctionKind::PlainDateTimeCompare => self.eval_plain_date_time_compare(args),
            TemporalFunctionKind::PlainDateTimePrototypeWith => {
                self.eval_plain_date_time_with(args, receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeWithPlainTime => {
                self.eval_plain_date_time_with_time(args, receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeWithCalendar => {
                self.eval_plain_date_time_with_calendar(args, receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeAdd => {
                self.eval_plain_date_time_add_subtract(args, receiver, false)
            }
            TemporalFunctionKind::PlainDateTimePrototypeSubtract => {
                self.eval_plain_date_time_add_subtract(args, receiver, true)
            }
            TemporalFunctionKind::PlainDateTimePrototypeUntil => {
                self.eval_plain_date_time_difference(args, receiver, false)
            }
            TemporalFunctionKind::PlainDateTimePrototypeSince => {
                self.eval_plain_date_time_difference(args, receiver, true)
            }
            TemporalFunctionKind::PlainDateTimePrototypeEquals => {
                self.eval_plain_date_time_equals(args, receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeRound => {
                self.eval_plain_date_time_round(args, receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeToZonedDateTime => {
                self.eval_plain_date_time_to_zoned(args, receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeToPlainDate => {
                let date = self.plain_date_time_receiver(receiver)?.to_plain_date();
                self.create_plain_date_value(date)
            }
            TemporalFunctionKind::PlainDateTimePrototypeToPlainTime => {
                let time = self.plain_date_time_receiver(receiver)?.to_plain_time();
                self.create_temporal_calendar_value(
                    TemporalValue::PlainTime(time),
                    TemporalFunctionKind::PlainTimeConstructor,
                )
            }
            TemporalFunctionKind::PlainDateTimePrototypeToString => {
                self.eval_plain_date_time_to_string(args, receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeToLocaleString => {
                self.plain_date_time_receiver(receiver)?;
                self.format_temporal_locale_string(receiver, args)
            }
            TemporalFunctionKind::PlainDateTimePrototypeToJson => {
                self.plain_date_time_default_string(receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeValueOf => Err(Error::type_error(
                "Temporal.PlainDateTime cannot be converted to a primitive",
            )),
            _ => Err(Error::runtime(
                "PlainDateTime function kind was not handled",
            )),
        }
    }

    fn eval_plain_date_time_accessor(
        &mut self,
        kind: TemporalFunctionKind,
        receiver: &Value,
    ) -> Option<Result<Value>> {
        let numeric: Option<fn(&PlainDateTime) -> i32> = match kind {
            TemporalFunctionKind::PlainDateTimePrototypeYear => {
                Some(PlainDateTime::year as fn(&PlainDateTime) -> i32)
            }
            TemporalFunctionKind::PlainDateTimePrototypeMonth => {
                Some(|value: &PlainDateTime| i32::from(value.month()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeDay => {
                Some(|value: &PlainDateTime| i32::from(value.day()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeHour => {
                Some(|value: &PlainDateTime| i32::from(value.hour()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeMinute => {
                Some(|value: &PlainDateTime| i32::from(value.minute()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeSecond => {
                Some(|value: &PlainDateTime| i32::from(value.second()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeMillisecond => {
                Some(|value: &PlainDateTime| i32::from(value.millisecond()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeMicrosecond => {
                Some(|value: &PlainDateTime| i32::from(value.microsecond()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeNanosecond => {
                Some(|value: &PlainDateTime| i32::from(value.nanosecond()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeDayOfWeek => {
                Some(|value: &PlainDateTime| i32::from(value.day_of_week()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeDayOfYear => {
                Some(|value: &PlainDateTime| i32::from(value.day_of_year()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeDaysInWeek => {
                Some(|value: &PlainDateTime| i32::from(value.days_in_week()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeDaysInMonth => {
                Some(|value: &PlainDateTime| i32::from(value.days_in_month()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeDaysInYear => {
                Some(|value: &PlainDateTime| i32::from(value.days_in_year()))
            }
            TemporalFunctionKind::PlainDateTimePrototypeMonthsInYear => {
                Some(|value: &PlainDateTime| i32::from(value.months_in_year()))
            }
            _ => None,
        };
        if let Some(getter) = numeric {
            return Some(self.plain_date_time_numeric(receiver, getter));
        }
        match kind {
            TemporalFunctionKind::PlainDateTimePrototypeMonthCode => Some(
                self.plain_date_time_receiver(receiver)
                    .and_then(|value| self.heap_string_value(value.month_code().as_str())),
            ),
            TemporalFunctionKind::PlainDateTimePrototypeCalendarId => Some(
                self.plain_date_time_receiver(receiver)
                    .and_then(|value| self.heap_string_value(value.calendar().identifier())),
            ),
            TemporalFunctionKind::PlainDateTimePrototypeEra => {
                Some(self.plain_date_time_receiver(receiver).and_then(|value| {
                    value.era().map_or(Ok(Value::Undefined), |era| {
                        self.heap_string_value(era.as_str())
                    })
                }))
            }
            TemporalFunctionKind::PlainDateTimePrototypeEraYear => {
                Some(self.plain_date_time_optional_numeric(receiver, PlainDateTime::era_year))
            }
            TemporalFunctionKind::PlainDateTimePrototypeWeekOfYear => {
                Some(self.plain_date_time_optional_numeric(receiver, PlainDateTime::week_of_year))
            }
            TemporalFunctionKind::PlainDateTimePrototypeYearOfWeek => {
                Some(self.plain_date_time_optional_numeric(receiver, PlainDateTime::year_of_week))
            }
            TemporalFunctionKind::PlainDateTimePrototypeInLeapYear => Some(
                self.plain_date_time_receiver(receiver)
                    .map(|value| Value::Bool(value.in_leap_year())),
            ),
            _ => None,
        }
    }

    fn plain_date_time_from_value(&mut self, value: Option<&Value>) -> Result<PlainDateTime> {
        let input = self.prepare_plain_date_time_input(value)?;
        Self::resolve_plain_date_time_input(input, Overflow::Constrain)
    }

    fn prepare_plain_date_time_input(
        &mut self,
        value: Option<&Value>,
    ) -> Result<PlainDateTimeInput> {
        let Some(value) = value else {
            return Err(Error::type_error(
                "Temporal.PlainDateTime requires an argument",
            ));
        };
        if let Value::Object(id) = value {
            match self.objects.temporal_value(*id)? {
                Some(TemporalValue::PlainDateTime(date_time)) => {
                    return Ok(PlainDateTimeInput::Resolved(date_time.clone()));
                }
                Some(TemporalValue::PlainDate(date)) => {
                    let result =
                        PlainDateTime::from_date_and_time(date.clone(), PlainTime::default())
                            .map_err(temporal_error)?;
                    return Ok(PlainDateTimeInput::Resolved(result));
                }
                Some(TemporalValue::ZonedDateTime(zoned)) => {
                    return Ok(PlainDateTimeInput::Resolved(zoned.to_plain_date_time()));
                }
                Some(
                    TemporalValue::Duration(_)
                    | TemporalValue::Instant(_)
                    | TemporalValue::PlainMonthDay(_)
                    | TemporalValue::PlainTime(_)
                    | TemporalValue::PlainYearMonth(_),
                )
                | None => {}
            }
        }
        if let Some(text) = value.string_text() {
            return Ok(PlainDateTimeInput::String(text.to_owned()));
        }
        let Value::Object(_) = value else {
            return Err(Error::type_error(
                "PlainDateTime input must be a string or object",
            ));
        };
        let calendar_value = self.get_named(value, "calendar")?;
        let calendar = self.temporal_calendar(Some(&calendar_value))?;
        let day = self.plain_date_required_i64(value, "day")?;
        let (era, era_year) = self.temporal_calendar_era_fields(value, &calendar)?;
        let hour = self.plain_date_optional_i64(value, "hour")?;
        let microsecond = self.plain_date_optional_i64(value, "microsecond")?;
        let millisecond = self.plain_date_optional_i64(value, "millisecond")?;
        let minute = self.plain_date_optional_i64(value, "minute")?;
        let month = self.plain_date_optional_i64(value, "month")?;
        let month_code_value = self.get_named(value, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_date_month_code(&month_code_value)?)
        };
        let nanosecond = self.plain_date_optional_i64(value, "nanosecond")?;
        let second = self.plain_date_optional_i64(value, "second")?;
        let year = self.plain_date_optional_i64(value, "year")?;
        Ok(PlainDateTimeInput::Fields(PlainDateTimeFields {
            calendar,
            day,
            era,
            era_year,
            hour,
            microsecond,
            millisecond,
            minute,
            month,
            month_code,
            nanosecond,
            second,
            year,
        }))
    }

    fn resolve_plain_date_time_input(
        input: PlainDateTimeInput,
        overflow: Overflow,
    ) -> Result<PlainDateTime> {
        match input {
            PlainDateTimeInput::Resolved(date_time) => Ok(date_time),
            PlainDateTimeInput::String(text) => {
                PlainDateTime::from_utf8(text.as_bytes()).map_err(temporal_error)
            }
            PlainDateTimeInput::Fields(fields) => {
                let year = fields
                    .year
                    .map(|value| {
                        value
                            .to_i32()
                            .ok_or_else(|| Self::plain_date_time_range("year is out of range"))
                    })
                    .transpose()?;
                let era_year = fields
                    .era_year
                    .map(|value| {
                        value
                            .to_i32()
                            .ok_or_else(|| Self::plain_date_time_range("eraYear is out of range"))
                    })
                    .transpose()?;
                let calendar_fields = CalendarFields::new()
                    .with_era(fields.era)
                    .with_era_year(era_year)
                    .with_optional_year(year)
                    .with_optional_month(
                        fields
                            .month
                            .map(|value| Self::plain_date_u8_field(value, "month", overflow))
                            .transpose()?,
                    )
                    .with_optional_month_code(fields.month_code)
                    .with_day(Self::plain_date_u8_field(fields.day, "day", overflow)?);
                let time = PartialTime::new()
                    .with_hour(Self::plain_date_time_u8_field(
                        fields.hour,
                        "hour",
                        23,
                        overflow,
                    )?)
                    .with_microsecond(Self::plain_date_time_u16_field(
                        fields.microsecond,
                        "microsecond",
                        overflow,
                    )?)
                    .with_millisecond(Self::plain_date_time_u16_field(
                        fields.millisecond,
                        "millisecond",
                        overflow,
                    )?)
                    .with_minute(Self::plain_date_time_u8_field(
                        fields.minute,
                        "minute",
                        59,
                        overflow,
                    )?)
                    .with_nanosecond(Self::plain_date_time_u16_field(
                        fields.nanosecond,
                        "nanosecond",
                        overflow,
                    )?)
                    .with_second(Self::plain_date_time_u8_field(
                        fields.second,
                        "second",
                        59,
                        overflow,
                    )?);
                PlainDateTime::from_partial(
                    PartialDateTime {
                        fields: DateTimeFields {
                            calendar_fields,
                            time,
                        },
                        calendar: fields.calendar,
                    },
                    Some(overflow),
                )
                .map_err(temporal_error)
            }
        }
    }

    pub(super) fn plain_date_time_u8_field(
        value: Option<i64>,
        name: &str,
        maximum: u8,
        overflow: Overflow,
    ) -> Result<Option<u8>> {
        value
            .map(|value| {
                if value < 0 {
                    return Err(Self::plain_date_time_range(format!("{name} is invalid")));
                }
                let normalized = match overflow {
                    Overflow::Constrain => value.min(i64::from(maximum)),
                    Overflow::Reject => value,
                };
                normalized
                    .to_u8()
                    .ok_or_else(|| Self::plain_date_time_range(format!("{name} is invalid")))
            })
            .transpose()
    }

    pub(super) fn plain_date_time_u16_field(
        value: Option<i64>,
        name: &str,
        overflow: Overflow,
    ) -> Result<Option<u16>> {
        value
            .map(|value| {
                if value < 0 {
                    return Err(Self::plain_date_time_range(format!("{name} is invalid")));
                }
                let normalized = match overflow {
                    Overflow::Constrain => value.min(999),
                    Overflow::Reject => value,
                };
                normalized
                    .to_u16()
                    .ok_or_else(|| Self::plain_date_time_range(format!("{name} is invalid")))
            })
            .transpose()
    }

    pub(super) fn plain_date_time_range(message: impl Into<String>) -> Error {
        Error::exception(
            ErrorName::RangeError,
            format!("PlainDateTime {}", message.into()),
        )
    }

    fn eval_plain_date_time_compare(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let one = self.plain_date_time_from_value(values.first())?;
        let two = self.plain_date_time_from_value(values.get(1))?;
        let result = match one.compare_iso(&two) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        };
        Ok(Value::Number(result))
    }

    fn eval_plain_date_time_with_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let time = match args.as_slice().first() {
            Some(Value::Undefined) | None => None,
            Some(value) => Some(self.plain_time_from_value(value)?),
        };
        let result = date_time.with_time(time).map_err(temporal_error)?;
        self.create_plain_date_time_value(result)
    }

    fn eval_plain_date_time_with_calendar(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let Some(value) = args.as_slice().first() else {
            return Err(Error::type_error("withCalendar requires an argument"));
        };
        if matches!(value, Value::Undefined) {
            return Err(Error::type_error("withCalendar requires an argument"));
        }
        let calendar = self.temporal_calendar(Some(value))?;
        self.create_plain_date_time_value(date_time.with_calendar(calendar))
    }

    fn eval_plain_date_time_add_subtract(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        subtract: bool,
    ) -> Result<Value> {
        let values = args.as_slice();
        let date_time = self.plain_date_time_receiver(receiver)?;
        let duration = self.duration_from_value(values.first())?;
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let result = if subtract {
            date_time.subtract(&duration, Some(overflow))
        } else {
            date_time.add(&duration, Some(overflow))
        }
        .map_err(temporal_error)?;
        self.create_plain_date_time_value(result)
    }

    fn eval_plain_date_time_difference(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        since: bool,
    ) -> Result<Value> {
        let values = args.as_slice();
        let date_time = self.plain_date_time_receiver(receiver)?;
        let other = self.plain_date_time_from_value(values.first())?;
        let settings = self.plain_date_difference_settings(values.get(1))?;
        let result = if since {
            date_time.since(&other, settings)
        } else {
            date_time.until(&other, settings)
        }
        .map_err(temporal_error)?;
        self.create_duration_value(result)
    }

    fn eval_plain_date_time_equals(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let other = self.plain_date_time_from_value(args.as_slice().first())?;
        Ok(Value::Bool(
            date_time.compare_iso(&other) == Ordering::Equal
                && date_time.calendar().identifier() == other.calendar().identifier(),
        ))
    }

    fn eval_plain_date_time_to_zoned(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let values = args.as_slice();
        let Some(value) = values.first() else {
            return Err(Error::type_error("toZonedDateTime requires a time zone"));
        };
        let Some(text) = value.string_text() else {
            return Err(Error::type_error("Temporal time zone must be a string"));
        };
        let disambiguation = self.plain_date_time_disambiguation(values.get(1))?;
        let zone = TimeZone::try_from_str(text).map_err(temporal_error)?;
        let result = date_time
            .to_zoned_date_time(zone, disambiguation)
            .map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::ZonedDateTime(result),
            TemporalFunctionKind::ZonedDateTimeConstructor,
        )
    }

    fn plain_date_time_disambiguation(&mut self, value: Option<&Value>) -> Result<Disambiguation> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(Disambiguation::Compatible);
        };
        if !matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        ) {
            return Err(Error::type_error("Temporal options must be an object"));
        }
        let option = self.get_named(value, "disambiguation")?;
        if matches!(option, Value::Undefined) {
            return Ok(Disambiguation::Compatible);
        }
        let text = self.to_string(&option)?;
        Disambiguation::from_str(&text).map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                format!("Invalid Temporal disambiguation: {text}"),
            )
        })
    }

    fn plain_date_time_default_string(&mut self, receiver: &Value) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let text = date_time
            .to_ixdtf_string(ToStringRoundingOptions::default(), DisplayCalendar::Auto)
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    pub(super) fn plain_date_time_receiver(&self, value: &Value) -> Result<PlainDateTime> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(
                "Temporal.PlainDateTime method requires a PlainDateTime receiver",
            ));
        };
        match self.objects.temporal_value(*id)? {
            Some(TemporalValue::PlainDateTime(date_time)) => Ok(date_time.clone()),
            _ => Err(Error::type_error(
                "Temporal.PlainDateTime method requires a PlainDateTime receiver",
            )),
        }
    }

    pub(super) fn create_plain_date_time_value(&mut self, value: PlainDateTime) -> Result<Value> {
        self.create_temporal_calendar_value(
            TemporalValue::PlainDateTime(value),
            TemporalFunctionKind::PlainDateTimeConstructor,
        )
    }

    fn plain_date_time_numeric(
        &self,
        receiver: &Value,
        getter: fn(&PlainDateTime) -> i32,
    ) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        Ok(Value::Number(f64::from(getter(&date_time))))
    }

    fn plain_date_time_optional_numeric<T>(
        &self,
        receiver: &Value,
        getter: fn(&PlainDateTime) -> Option<T>,
    ) -> Result<Value>
    where
        T: ToPrimitive,
    {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let Some(value) = getter(&date_time) else {
            return Ok(Value::Undefined);
        };
        value
            .to_f64()
            .map(Value::Number)
            .ok_or_else(|| Error::runtime("PlainDateTime field cannot become Number"))
    }
}
