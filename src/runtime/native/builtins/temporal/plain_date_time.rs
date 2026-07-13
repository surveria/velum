use std::cmp::Ordering;

use num_traits::ToPrimitive;
use temporal_rs::{
    PlainDateTime, TimeZone,
    options::{Disambiguation, DisplayCalendar, ToStringRoundingOptions},
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::Value,
};

use super::temporal_error;

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
                let date_time = self.plain_date_time_from_value(args.as_slice().first())?;
                self.create_plain_date_time_value(date_time)
            }
            TemporalFunctionKind::PlainDateTimeCompare => self.eval_plain_date_time_compare(args),
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
            TemporalFunctionKind::PlainDateTimePrototypeToString
            | TemporalFunctionKind::PlainDateTimePrototypeToLocaleString
            | TemporalFunctionKind::PlainDateTimePrototypeToJson => {
                self.plain_date_time_default_string(receiver)
            }
            TemporalFunctionKind::PlainDateTimePrototypeValueOf => Err(Error::type_error(
                "Temporal.PlainDateTime cannot be converted to a primitive",
            )),
            TemporalFunctionKind::PlainDateTimePrototypeWith
            | TemporalFunctionKind::PlainDateTimePrototypeRound => Err(Error::runtime(
                "Temporal.PlainDateTime method is not implemented yet",
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

    fn plain_date_time_from_value(&self, value: Option<&Value>) -> Result<PlainDateTime> {
        let Some(value) = value else {
            return Err(Error::type_error(
                "Temporal.PlainDateTime requires an argument",
            ));
        };
        if let Ok(date_time) = self.plain_date_time_receiver(value) {
            return Ok(date_time);
        }
        if let Some(text) = value.string_text() {
            return PlainDateTime::from_utf8(text.as_bytes()).map_err(temporal_error);
        }
        Err(Error::type_error(
            "PlainDateTime input must be a string or PlainDateTime",
        ))
    }

    fn eval_plain_date_time_compare(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
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
        &self,
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
        let Some(value) = args.as_slice().first() else {
            return Err(Error::type_error("toZonedDateTime requires a time zone"));
        };
        let Some(text) = value.string_text() else {
            return Err(Error::type_error("Temporal time zone must be a string"));
        };
        let zone = TimeZone::try_from_str(text).map_err(temporal_error)?;
        let result = date_time
            .to_zoned_date_time(zone, Disambiguation::Compatible)
            .map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::ZonedDateTime(result),
            TemporalFunctionKind::ZonedDateTimeConstructor,
        )
    }

    fn plain_date_time_default_string(&mut self, receiver: &Value) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let text = date_time
            .to_ixdtf_string(ToStringRoundingOptions::default(), DisplayCalendar::Auto)
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    fn plain_date_time_receiver(&self, value: &Value) -> Result<PlainDateTime> {
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

    fn create_plain_date_time_value(&mut self, value: PlainDateTime) -> Result<Value> {
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
