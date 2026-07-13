use std::{cmp::Ordering, str::FromStr};

use num_traits::ToPrimitive;
use temporal_rs::{
    TimeZone,
    options::{
        DisplayCalendar, DisplayOffset, DisplayTimeZone, OffsetDisambiguation,
        ToStringRoundingOptions,
    },
    provider::TransitionDirection,
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::{ErrorName, JsBigInt, Value},
};

use super::temporal_error;

pub(super) const STATIC_METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("from", TemporalFunctionKind::ZonedDateTimeFrom),
    ("compare", TemporalFunctionKind::ZonedDateTimeCompare),
];

pub(super) const ACCESSORS: &[(&str, TemporalFunctionKind)] = &[
    (
        "epochMilliseconds",
        TemporalFunctionKind::ZonedDateTimePrototypeEpochMilliseconds,
    ),
    (
        "epochNanoseconds",
        TemporalFunctionKind::ZonedDateTimePrototypeEpochNanoseconds,
    ),
    (
        "timeZoneId",
        TemporalFunctionKind::ZonedDateTimePrototypeTimeZoneId,
    ),
    (
        "calendarId",
        TemporalFunctionKind::ZonedDateTimePrototypeCalendarId,
    ),
    ("year", TemporalFunctionKind::ZonedDateTimePrototypeYear),
    ("month", TemporalFunctionKind::ZonedDateTimePrototypeMonth),
    (
        "monthCode",
        TemporalFunctionKind::ZonedDateTimePrototypeMonthCode,
    ),
    ("day", TemporalFunctionKind::ZonedDateTimePrototypeDay),
    ("hour", TemporalFunctionKind::ZonedDateTimePrototypeHour),
    ("minute", TemporalFunctionKind::ZonedDateTimePrototypeMinute),
    ("second", TemporalFunctionKind::ZonedDateTimePrototypeSecond),
    (
        "millisecond",
        TemporalFunctionKind::ZonedDateTimePrototypeMillisecond,
    ),
    (
        "microsecond",
        TemporalFunctionKind::ZonedDateTimePrototypeMicrosecond,
    ),
    (
        "nanosecond",
        TemporalFunctionKind::ZonedDateTimePrototypeNanosecond,
    ),
    ("era", TemporalFunctionKind::ZonedDateTimePrototypeEra),
    (
        "eraYear",
        TemporalFunctionKind::ZonedDateTimePrototypeEraYear,
    ),
    (
        "dayOfWeek",
        TemporalFunctionKind::ZonedDateTimePrototypeDayOfWeek,
    ),
    (
        "dayOfYear",
        TemporalFunctionKind::ZonedDateTimePrototypeDayOfYear,
    ),
    (
        "weekOfYear",
        TemporalFunctionKind::ZonedDateTimePrototypeWeekOfYear,
    ),
    (
        "yearOfWeek",
        TemporalFunctionKind::ZonedDateTimePrototypeYearOfWeek,
    ),
    (
        "hoursInDay",
        TemporalFunctionKind::ZonedDateTimePrototypeHoursInDay,
    ),
    (
        "daysInWeek",
        TemporalFunctionKind::ZonedDateTimePrototypeDaysInWeek,
    ),
    (
        "daysInMonth",
        TemporalFunctionKind::ZonedDateTimePrototypeDaysInMonth,
    ),
    (
        "daysInYear",
        TemporalFunctionKind::ZonedDateTimePrototypeDaysInYear,
    ),
    (
        "monthsInYear",
        TemporalFunctionKind::ZonedDateTimePrototypeMonthsInYear,
    ),
    (
        "inLeapYear",
        TemporalFunctionKind::ZonedDateTimePrototypeInLeapYear,
    ),
    ("offset", TemporalFunctionKind::ZonedDateTimePrototypeOffset),
    (
        "offsetNanoseconds",
        TemporalFunctionKind::ZonedDateTimePrototypeOffsetNanoseconds,
    ),
];

pub(super) const METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("add", TemporalFunctionKind::ZonedDateTimePrototypeAdd),
    (
        "subtract",
        TemporalFunctionKind::ZonedDateTimePrototypeSubtract,
    ),
    ("with", TemporalFunctionKind::ZonedDateTimePrototypeWith),
    ("until", TemporalFunctionKind::ZonedDateTimePrototypeUntil),
    ("since", TemporalFunctionKind::ZonedDateTimePrototypeSince),
    ("round", TemporalFunctionKind::ZonedDateTimePrototypeRound),
    ("equals", TemporalFunctionKind::ZonedDateTimePrototypeEquals),
    (
        "startOfDay",
        TemporalFunctionKind::ZonedDateTimePrototypeStartOfDay,
    ),
    (
        "getTimeZoneTransition",
        TemporalFunctionKind::ZonedDateTimePrototypeGetTimeZoneTransition,
    ),
    (
        "withPlainTime",
        TemporalFunctionKind::ZonedDateTimePrototypeWithPlainTime,
    ),
    (
        "withTimeZone",
        TemporalFunctionKind::ZonedDateTimePrototypeWithTimeZone,
    ),
    (
        "withCalendar",
        TemporalFunctionKind::ZonedDateTimePrototypeWithCalendar,
    ),
    (
        "toInstant",
        TemporalFunctionKind::ZonedDateTimePrototypeToInstant,
    ),
    (
        "toPlainDate",
        TemporalFunctionKind::ZonedDateTimePrototypeToPlainDate,
    ),
    (
        "toPlainTime",
        TemporalFunctionKind::ZonedDateTimePrototypeToPlainTime,
    ),
    (
        "toPlainDateTime",
        TemporalFunctionKind::ZonedDateTimePrototypeToPlainDateTime,
    ),
    (
        "toString",
        TemporalFunctionKind::ZonedDateTimePrototypeToString,
    ),
    (
        "toLocaleString",
        TemporalFunctionKind::ZonedDateTimePrototypeToLocaleString,
    ),
    ("toJSON", TemporalFunctionKind::ZonedDateTimePrototypeToJson),
    (
        "valueOf",
        TemporalFunctionKind::ZonedDateTimePrototypeValueOf,
    ),
];

impl Context {
    pub(super) fn eval_zoned_date_time_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if let Some(result) = self.eval_zoned_date_time_accessor(kind, receiver) {
            return result;
        }
        match kind {
            TemporalFunctionKind::ZonedDateTimeConstructor => Err(Error::type_error(
                "Temporal.ZonedDateTime constructor requires 'new'",
            )),
            TemporalFunctionKind::ZonedDateTimeFrom => self.eval_zoned_date_time_from(args),
            TemporalFunctionKind::ZonedDateTimeCompare => self.eval_zoned_date_time_compare(args),
            TemporalFunctionKind::ZonedDateTimePrototypeAdd => {
                self.eval_zoned_date_time_add_subtract(args, receiver, false)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeSubtract => {
                self.eval_zoned_date_time_add_subtract(args, receiver, true)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeWith => {
                self.eval_zoned_date_time_with(args, receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeUntil => {
                self.eval_zoned_date_time_difference(args, receiver, false)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeSince => {
                self.eval_zoned_date_time_difference(args, receiver, true)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeRound => {
                self.eval_zoned_date_time_round(args, receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeEquals => {
                self.eval_zoned_date_time_equals(args, receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeStartOfDay => {
                let result = self
                    .zoned_date_time_receiver(receiver)?
                    .start_of_day()
                    .map_err(temporal_error)?;
                self.create_zoned_date_time_value(result)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeGetTimeZoneTransition => {
                self.eval_zoned_date_time_transition(args, receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeWithPlainTime => {
                self.eval_zoned_date_time_with_plain_time(args, receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeWithTimeZone => {
                self.eval_zoned_date_time_with_time_zone(args, receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeWithCalendar => {
                self.eval_zoned_date_time_with_calendar(args, receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeToInstant => {
                let instant = self.zoned_date_time_receiver(receiver)?.to_instant();
                self.create_temporal_calendar_value(
                    TemporalValue::Instant(instant),
                    TemporalFunctionKind::InstantConstructor,
                )
            }
            TemporalFunctionKind::ZonedDateTimePrototypeToPlainDate => {
                let date = self.zoned_date_time_receiver(receiver)?.to_plain_date();
                self.create_plain_date_value(date)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeToPlainTime => {
                let time = self.zoned_date_time_receiver(receiver)?.to_plain_time();
                self.create_temporal_calendar_value(
                    TemporalValue::PlainTime(time),
                    TemporalFunctionKind::PlainTimeConstructor,
                )
            }
            TemporalFunctionKind::ZonedDateTimePrototypeToPlainDateTime => {
                let date_time = self
                    .zoned_date_time_receiver(receiver)?
                    .to_plain_date_time();
                self.create_plain_date_time_value(date_time)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeToString => {
                self.eval_zoned_date_time_to_string(args, receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeToLocaleString => {
                self.format_temporal_locale_string(receiver, args)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeToJson => {
                self.zoned_date_time_default_string(receiver)
            }
            TemporalFunctionKind::ZonedDateTimePrototypeValueOf => Err(Error::type_error(
                "Temporal.ZonedDateTime cannot be converted to a primitive",
            )),
            _ => Err(Error::runtime(
                "ZonedDateTime function kind was not handled",
            )),
        }
    }

    fn eval_zoned_date_time_accessor(
        &mut self,
        kind: TemporalFunctionKind,
        receiver: &Value,
    ) -> Option<Result<Value>> {
        if !Self::zoned_accessor_kind(kind) {
            return None;
        }
        let zoned = match self.zoned_date_time_receiver(receiver) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        };
        let result = match kind {
            TemporalFunctionKind::ZonedDateTimePrototypeEpochMilliseconds => {
                Self::zoned_number(&zoned.epoch_milliseconds())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeEpochNanoseconds => {
                Self::zoned_bigint(zoned.epoch_nanoseconds().as_i128())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeTimeZoneId => zoned
                .time_zone()
                .identifier()
                .map_err(temporal_error)
                .and_then(|text| self.heap_string_value(&text)),
            TemporalFunctionKind::ZonedDateTimePrototypeCalendarId => {
                self.heap_string_value(zoned.calendar().identifier())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeYear => Self::zoned_number(&zoned.year()),
            TemporalFunctionKind::ZonedDateTimePrototypeMonth => Self::zoned_number(&zoned.month()),
            TemporalFunctionKind::ZonedDateTimePrototypeMonthCode => {
                self.heap_string_value(zoned.month_code().as_str())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeDay => Self::zoned_number(&zoned.day()),
            TemporalFunctionKind::ZonedDateTimePrototypeHour => Self::zoned_number(&zoned.hour()),
            TemporalFunctionKind::ZonedDateTimePrototypeMinute => {
                Self::zoned_number(&zoned.minute())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeSecond => {
                Self::zoned_number(&zoned.second())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeMillisecond => {
                Self::zoned_number(&zoned.millisecond())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeMicrosecond => {
                Self::zoned_number(&zoned.microsecond())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeNanosecond => {
                Self::zoned_number(&zoned.nanosecond())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeEra => zoned.era().map_or_else(
                || Ok(Value::Undefined),
                |era| self.heap_string_value(era.as_str()),
            ),
            TemporalFunctionKind::ZonedDateTimePrototypeEraYear => {
                Self::zoned_optional_number(zoned.era_year())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeDayOfWeek => {
                Self::zoned_number(&zoned.day_of_week())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeDayOfYear => {
                Self::zoned_number(&zoned.day_of_year())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeWeekOfYear => {
                Self::zoned_optional_number(zoned.week_of_year())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeYearOfWeek => {
                Self::zoned_optional_number(zoned.year_of_week())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeHoursInDay => zoned
                .hours_in_day()
                .map(Value::Number)
                .map_err(temporal_error),
            TemporalFunctionKind::ZonedDateTimePrototypeDaysInWeek => {
                Self::zoned_number(&zoned.days_in_week())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeDaysInMonth => {
                Self::zoned_number(&zoned.days_in_month())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeDaysInYear => {
                Self::zoned_number(&zoned.days_in_year())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeMonthsInYear => {
                Self::zoned_number(&zoned.months_in_year())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeInLeapYear => {
                Ok(Value::Bool(zoned.in_leap_year()))
            }
            TemporalFunctionKind::ZonedDateTimePrototypeOffset => {
                self.heap_string_value(&zoned.offset())
            }
            TemporalFunctionKind::ZonedDateTimePrototypeOffsetNanoseconds => {
                Self::zoned_number(&zoned.offset_nanoseconds())
            }
            _ => return None,
        };
        Some(result)
    }

    fn eval_zoned_date_time_from(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let zoned = self.zoned_date_time_argument_with_options(
            values.first(),
            values.get(1),
            OffsetDisambiguation::Reject,
        )?;
        self.create_zoned_date_time_value(zoned)
    }

    fn eval_zoned_date_time_compare(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let one = self.zoned_date_time_argument(values.first())?;
        let two = self.zoned_date_time_argument(values.get(1))?;
        let result = match one.compare_instant(&two) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        };
        Ok(Value::Number(result))
    }

    fn eval_zoned_date_time_add_subtract(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        subtract: bool,
    ) -> Result<Value> {
        let values = args.as_slice();
        let zoned = self.zoned_date_time_receiver(receiver)?;
        let duration = self.duration_from_value(values.first())?;
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let result = if subtract {
            zoned.subtract(&duration, Some(overflow))
        } else {
            zoned.add(&duration, Some(overflow))
        }
        .map_err(temporal_error)?;
        self.create_zoned_date_time_value(result)
    }

    fn eval_zoned_date_time_difference(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        since: bool,
    ) -> Result<Value> {
        let values = args.as_slice();
        let zoned = self.zoned_date_time_receiver(receiver)?;
        let other = self.zoned_date_time_argument(values.first())?;
        let settings = self.plain_date_difference_settings(values.get(1))?;
        let result = if since {
            zoned.since(&other, settings)
        } else {
            zoned.until(&other, settings)
        }
        .map_err(temporal_error)?;
        self.create_duration_value(result)
    }

    fn eval_zoned_date_time_round(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let zoned = self.zoned_date_time_receiver(receiver)?;
        let options = self.plain_date_time_rounding_options(args.as_slice().first())?;
        let result = zoned.round(options).map_err(temporal_error)?;
        self.create_zoned_date_time_value(result)
    }

    fn eval_zoned_date_time_equals(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let zoned = self.zoned_date_time_receiver(receiver)?;
        let other = self.zoned_date_time_argument(args.as_slice().first())?;
        Ok(Value::Bool(zoned.equals(&other).map_err(temporal_error)?))
    }

    fn eval_zoned_date_time_transition(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let value = args
            .as_slice()
            .first()
            .ok_or_else(|| Error::type_error("getTimeZoneTransition requires options"))?;
        let text = if let Some(text) = value.string_text() {
            text.to_owned()
        } else {
            if !Self::zoned_object(value) {
                return Err(Error::type_error(
                    "getTimeZoneTransition options must be a string or object",
                ));
            }
            let direction = self.get_named(value, "direction")?;
            self.to_string(&direction)?
        };
        let direction = TransitionDirection::from_str(&text).map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                "Invalid time zone transition direction",
            )
        })?;
        let result = self
            .zoned_date_time_receiver(receiver)?
            .get_time_zone_transition(direction)
            .map_err(temporal_error)?;
        result.map_or(Ok(Value::Null), |value| {
            self.create_zoned_date_time_value(value)
        })
    }

    fn eval_zoned_date_time_with_plain_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let value = args.as_slice().first();
        let time = match value {
            None | Some(Value::Undefined) => None,
            Some(value) => Some(self.plain_time_from_value(value)?),
        };
        let result = self
            .zoned_date_time_receiver(receiver)?
            .with_plain_time(time)
            .map_err(temporal_error)?;
        self.create_zoned_date_time_value(result)
    }

    fn eval_zoned_date_time_with_time_zone(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let time_zone = Self::zoned_time_zone(args.as_slice().first())?;
        let result = self
            .zoned_date_time_receiver(receiver)?
            .with_timezone(time_zone)
            .map_err(temporal_error)?;
        self.create_zoned_date_time_value(result)
    }

    fn eval_zoned_date_time_with_calendar(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let value = args
            .as_slice()
            .first()
            .filter(|value| !matches!(value, Value::Undefined))
            .ok_or_else(|| Error::type_error("withCalendar requires a calendar"))?;
        let calendar = self.temporal_calendar(Some(value))?;
        let result = self
            .zoned_date_time_receiver(receiver)?
            .with_calendar(calendar);
        self.create_zoned_date_time_value(result)
    }

    fn eval_zoned_date_time_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let value = args.as_slice().first();
        let options = self.zoned_string_options(value)?;
        let text = self
            .zoned_date_time_receiver(receiver)?
            .to_ixdtf_string(
                options.offset,
                options.time_zone,
                options.calendar,
                options.rounding,
            )
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    fn zoned_date_time_default_string(&mut self, receiver: &Value) -> Result<Value> {
        let text = self
            .zoned_date_time_receiver(receiver)?
            .to_ixdtf_string(
                DisplayOffset::Auto,
                DisplayTimeZone::Auto,
                DisplayCalendar::Auto,
                ToStringRoundingOptions::default(),
            )
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    fn zoned_time_zone(value: Option<&Value>) -> Result<TimeZone> {
        let value = value.ok_or_else(|| Error::type_error("Temporal time zone is required"))?;
        let text = value
            .string_text()
            .ok_or_else(|| Error::type_error("Temporal time zone must be a string"))?;
        TimeZone::try_from_str(text).map_err(temporal_error)
    }

    fn zoned_number<T: ToPrimitive>(value: &T) -> Result<Value> {
        value
            .to_f64()
            .map(Value::Number)
            .ok_or_else(|| Error::runtime("ZonedDateTime field cannot become Number"))
    }

    fn zoned_optional_number<T: ToPrimitive>(value: Option<T>) -> Result<Value> {
        value.map_or(Ok(Value::Undefined), |number| Self::zoned_number(&number))
    }

    fn zoned_bigint(value: i128) -> Result<Value> {
        JsBigInt::parse_string(&value.to_string())
            .map(Value::BigInt)
            .ok_or_else(|| Error::runtime("ZonedDateTime nanoseconds cannot become BigInt"))
    }

    const fn zoned_object(value: &Value) -> bool {
        matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        )
    }

    const fn zoned_accessor_kind(kind: TemporalFunctionKind) -> bool {
        matches!(
            kind,
            TemporalFunctionKind::ZonedDateTimePrototypeEpochMilliseconds
                | TemporalFunctionKind::ZonedDateTimePrototypeEpochNanoseconds
                | TemporalFunctionKind::ZonedDateTimePrototypeTimeZoneId
                | TemporalFunctionKind::ZonedDateTimePrototypeCalendarId
                | TemporalFunctionKind::ZonedDateTimePrototypeYear
                | TemporalFunctionKind::ZonedDateTimePrototypeMonth
                | TemporalFunctionKind::ZonedDateTimePrototypeMonthCode
                | TemporalFunctionKind::ZonedDateTimePrototypeDay
                | TemporalFunctionKind::ZonedDateTimePrototypeHour
                | TemporalFunctionKind::ZonedDateTimePrototypeMinute
                | TemporalFunctionKind::ZonedDateTimePrototypeSecond
                | TemporalFunctionKind::ZonedDateTimePrototypeMillisecond
                | TemporalFunctionKind::ZonedDateTimePrototypeMicrosecond
                | TemporalFunctionKind::ZonedDateTimePrototypeNanosecond
                | TemporalFunctionKind::ZonedDateTimePrototypeEra
                | TemporalFunctionKind::ZonedDateTimePrototypeEraYear
                | TemporalFunctionKind::ZonedDateTimePrototypeDayOfWeek
                | TemporalFunctionKind::ZonedDateTimePrototypeDayOfYear
                | TemporalFunctionKind::ZonedDateTimePrototypeWeekOfYear
                | TemporalFunctionKind::ZonedDateTimePrototypeYearOfWeek
                | TemporalFunctionKind::ZonedDateTimePrototypeHoursInDay
                | TemporalFunctionKind::ZonedDateTimePrototypeDaysInWeek
                | TemporalFunctionKind::ZonedDateTimePrototypeDaysInMonth
                | TemporalFunctionKind::ZonedDateTimePrototypeDaysInYear
                | TemporalFunctionKind::ZonedDateTimePrototypeMonthsInYear
                | TemporalFunctionKind::ZonedDateTimePrototypeInLeapYear
                | TemporalFunctionKind::ZonedDateTimePrototypeOffset
                | TemporalFunctionKind::ZonedDateTimePrototypeOffsetNanoseconds
        )
    }
}
