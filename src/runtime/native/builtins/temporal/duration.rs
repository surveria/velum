use std::{cmp::Ordering, str::FromStr};

use num_traits::ToPrimitive;
use temporal_rs::{
    Calendar, Duration, PlainDate, Sign,
    options::{
        DisplayCalendar, RelativeTo, RoundingIncrement, RoundingMode, RoundingOptions,
        ToStringRoundingOptions, Unit,
    },
    parsers::Precision,
    partial::PartialDuration,
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::{ErrorName, Value},
};

use super::temporal_error;

const DURATION_RECEIVER_ERROR: &str = "Temporal.Duration method requires a Duration receiver";
const DURATION_ARGUMENT_ERROR: &str = "Temporal.Duration argument must be a string or object";
const DURATION_CONSTRUCTOR_CALL_ERROR: &str = "Temporal.Duration constructor requires 'new'";
const DURATION_INTEGER_ERROR: &str = "Temporal.Duration fields must be finite integers";
const DURATION_OPTIONS_ERROR: &str = "Temporal.Duration options must be a string or object";

#[derive(Clone, Copy)]
enum DurationField {
    Years,
    Months,
    Weeks,
    Days,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
}

impl Context {
    pub(in crate::runtime::native) fn construct_temporal_duration(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let duration = Duration::new(
            self.duration_i64_argument(values.first())?,
            self.duration_i64_argument(values.get(1))?,
            self.duration_i64_argument(values.get(2))?,
            self.duration_i64_argument(values.get(3))?,
            self.duration_i64_argument(values.get(4))?,
            self.duration_i64_argument(values.get(5))?,
            self.duration_i64_argument(values.get(6))?,
            self.duration_i64_argument(values.get(7))?,
            self.duration_i128_argument(values.get(8))?,
            self.duration_i128_argument(values.get(9))?,
        )
        .map_err(temporal_error)?;
        self.create_duration_value(duration)
    }

    pub(in crate::runtime) fn eval_temporal_native_function_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match kind {
            TemporalFunctionKind::Constructor => {
                Err(Error::type_error(DURATION_CONSTRUCTOR_CALL_ERROR))
            }
            TemporalFunctionKind::From => self.eval_duration_from(args),
            TemporalFunctionKind::Compare => self.eval_duration_compare(args),
            TemporalFunctionKind::PrototypeYears => {
                self.duration_field_value(this_value, DurationField::Years)
            }
            TemporalFunctionKind::PrototypeMonths => {
                self.duration_field_value(this_value, DurationField::Months)
            }
            TemporalFunctionKind::PrototypeWeeks => {
                self.duration_field_value(this_value, DurationField::Weeks)
            }
            TemporalFunctionKind::PrototypeDays => {
                self.duration_field_value(this_value, DurationField::Days)
            }
            TemporalFunctionKind::PrototypeHours => {
                self.duration_field_value(this_value, DurationField::Hours)
            }
            TemporalFunctionKind::PrototypeMinutes => {
                self.duration_field_value(this_value, DurationField::Minutes)
            }
            TemporalFunctionKind::PrototypeSeconds => {
                self.duration_field_value(this_value, DurationField::Seconds)
            }
            TemporalFunctionKind::PrototypeMilliseconds => {
                self.duration_field_value(this_value, DurationField::Milliseconds)
            }
            TemporalFunctionKind::PrototypeMicroseconds => {
                self.duration_field_value(this_value, DurationField::Microseconds)
            }
            TemporalFunctionKind::PrototypeNanoseconds => {
                self.duration_field_value(this_value, DurationField::Nanoseconds)
            }
            TemporalFunctionKind::PrototypeSign => self.duration_sign_value(this_value),
            TemporalFunctionKind::PrototypeBlank => self.duration_blank_value(this_value),
            TemporalFunctionKind::PrototypeWith => self.eval_duration_with(args, this_value),
            TemporalFunctionKind::PrototypeNegated => {
                self.eval_duration_unary(this_value, Duration::negated)
            }
            TemporalFunctionKind::PrototypeAbs => {
                self.eval_duration_unary(this_value, Duration::abs)
            }
            TemporalFunctionKind::PrototypeAdd => {
                self.eval_duration_binary(args, this_value, Duration::add)
            }
            TemporalFunctionKind::PrototypeSubtract => {
                self.eval_duration_binary(args, this_value, Duration::subtract)
            }
            TemporalFunctionKind::PrototypeRound => self.eval_duration_round(args, this_value),
            TemporalFunctionKind::PrototypeTotal => self.eval_duration_total(args, this_value),
            TemporalFunctionKind::PrototypeToString => {
                self.eval_duration_to_string(args, this_value)
            }
            TemporalFunctionKind::PrototypeToJson
            | TemporalFunctionKind::PrototypeToLocaleString => {
                self.duration_default_string(this_value)
            }
            TemporalFunctionKind::PrototypeValueOf => Err(Error::type_error(
                "Temporal.Duration cannot be converted to a primitive",
            )),
            kind => self.eval_temporal_calendar_kind(kind, args, this_value),
        }
    }

    pub(super) fn create_duration_value(&mut self, duration: Duration) -> Result<Value> {
        let prototype = self.temporal_duration_constructor_prototype()?;
        self.objects.create_temporal_object(
            TemporalValue::Duration(duration),
            prototype,
            self.limits.max_objects,
        )
    }

    fn duration_receiver(&self, value: &Value) -> Result<Duration> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(DURATION_RECEIVER_ERROR));
        };
        match self.objects.temporal_value(*id)? {
            Some(TemporalValue::Duration(duration)) => Ok(*duration),
            _ => Err(Error::type_error(DURATION_RECEIVER_ERROR)),
        }
    }

    fn duration_i64_argument(&mut self, value: Option<&Value>) -> Result<i64> {
        let number = self.duration_number_argument(value)?;
        number
            .to_i64()
            .ok_or_else(|| Error::exception(ErrorName::RangeError, DURATION_INTEGER_ERROR))
    }

    fn duration_i128_argument(&mut self, value: Option<&Value>) -> Result<i128> {
        let number = self.duration_number_argument(value)?;
        number
            .to_i128()
            .ok_or_else(|| Error::exception(ErrorName::RangeError, DURATION_INTEGER_ERROR))
    }

    fn duration_number_argument(&mut self, value: Option<&Value>) -> Result<f64> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(0.0);
        };
        let number = self.to_number(value)?;
        if !number.is_finite() || number.fract() != 0.0 {
            return Err(Error::exception(
                ErrorName::RangeError,
                DURATION_INTEGER_ERROR,
            ));
        }
        Ok(number)
    }

    fn eval_duration_from(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let duration = self.duration_from_value(args.as_slice().first())?;
        self.create_duration_value(duration)
    }

    fn eval_duration_compare(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let one = self.duration_from_value(values.first())?;
        let two = self.duration_from_value(values.get(1))?;
        let relative_to = self.duration_relative_to_option(values.get(2))?;
        let ordering = one.compare(&two, relative_to).map_err(temporal_error)?;
        let result = match ordering {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        };
        Ok(Value::Number(result))
    }

    pub(super) fn duration_from_value(&mut self, value: Option<&Value>) -> Result<Duration> {
        let Some(value) = value else {
            return Err(Error::type_error(DURATION_ARGUMENT_ERROR));
        };
        if let Ok(duration) = self.duration_receiver(value) {
            return Ok(duration);
        }
        if let Some(text) = value.string_text() {
            return Duration::from_utf8(text.as_bytes()).map_err(temporal_error);
        }
        if !Self::is_object_value(value) {
            return Err(Error::type_error(DURATION_ARGUMENT_ERROR));
        }
        let partial = self.duration_partial_from_object(value)?;
        Duration::from_partial_duration(partial).map_err(temporal_error)
    }

    fn duration_partial_from_object(&mut self, value: &Value) -> Result<PartialDuration> {
        let mut partial = PartialDuration::empty();
        partial.days = self.optional_duration_i64_property(value, "days")?;
        partial.hours = self.optional_duration_i64_property(value, "hours")?;
        partial.microseconds = self.optional_duration_i128_property(value, "microseconds")?;
        partial.milliseconds = self.optional_duration_i64_property(value, "milliseconds")?;
        partial.minutes = self.optional_duration_i64_property(value, "minutes")?;
        partial.months = self.optional_duration_i64_property(value, "months")?;
        partial.nanoseconds = self.optional_duration_i128_property(value, "nanoseconds")?;
        partial.seconds = self.optional_duration_i64_property(value, "seconds")?;
        partial.weeks = self.optional_duration_i64_property(value, "weeks")?;
        partial.years = self.optional_duration_i64_property(value, "years")?;
        Ok(partial)
    }

    fn optional_duration_i64_property(
        &mut self,
        object: &Value,
        name: &str,
    ) -> Result<Option<i64>> {
        let value = self.get_named(object, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        self.duration_i64_argument(Some(&value)).map(Some)
    }

    fn optional_duration_i128_property(
        &mut self,
        object: &Value,
        name: &str,
    ) -> Result<Option<i128>> {
        let value = self.get_named(object, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        self.duration_i128_argument(Some(&value)).map(Some)
    }

    fn duration_field_value(&self, receiver: &Value, field: DurationField) -> Result<Value> {
        let duration = self.duration_receiver(receiver)?;
        let number = match field {
            DurationField::Years => duration.years().to_f64(),
            DurationField::Months => duration.months().to_f64(),
            DurationField::Weeks => duration.weeks().to_f64(),
            DurationField::Days => duration.days().to_f64(),
            DurationField::Hours => duration.hours().to_f64(),
            DurationField::Minutes => duration.minutes().to_f64(),
            DurationField::Seconds => duration.seconds().to_f64(),
            DurationField::Milliseconds => duration.milliseconds().to_f64(),
            DurationField::Microseconds => duration.microseconds().to_f64(),
            DurationField::Nanoseconds => duration.nanoseconds().to_f64(),
        }
        .ok_or_else(|| Error::runtime("Temporal.Duration field cannot be represented as Number"))?;
        Ok(Value::Number(number))
    }

    fn duration_sign_value(&self, receiver: &Value) -> Result<Value> {
        let sign = match self.duration_receiver(receiver)?.sign() {
            Sign::Negative => -1.0,
            Sign::Zero => 0.0,
            Sign::Positive => 1.0,
        };
        Ok(Value::Number(sign))
    }

    fn duration_blank_value(&self, receiver: &Value) -> Result<Value> {
        Ok(Value::Bool(self.duration_receiver(receiver)?.is_zero()))
    }

    fn eval_duration_unary(
        &mut self,
        receiver: &Value,
        operation: fn(&Duration) -> Duration,
    ) -> Result<Value> {
        let duration = self.duration_receiver(receiver)?;
        self.create_duration_value(operation(&duration))
    }

    fn eval_duration_binary(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        operation: fn(&Duration, &Duration) -> temporal_rs::TemporalResult<Duration>,
    ) -> Result<Value> {
        let duration = self.duration_receiver(receiver)?;
        let other = self.duration_from_value(args.as_slice().first())?;
        let result = operation(&duration, &other).map_err(temporal_error)?;
        self.create_duration_value(result)
    }

    fn eval_duration_with(&mut self, args: RuntimeCallArgs<'_>, receiver: &Value) -> Result<Value> {
        let duration = self.duration_receiver(receiver)?;
        let Some(value) = args
            .as_slice()
            .first()
            .filter(|value| Self::is_object_value(value))
        else {
            return Err(Error::type_error(DURATION_ARGUMENT_ERROR));
        };
        let partial = self.duration_partial_from_object(value)?;
        if partial == PartialDuration::empty() {
            return Err(Error::type_error(
                "Temporal.Duration.with requires at least one field",
            ));
        }
        let result = Duration::new(
            partial.years.unwrap_or_else(|| duration.years()),
            partial.months.unwrap_or_else(|| duration.months()),
            partial.weeks.unwrap_or_else(|| duration.weeks()),
            partial.days.unwrap_or_else(|| duration.days()),
            partial.hours.unwrap_or_else(|| duration.hours()),
            partial.minutes.unwrap_or_else(|| duration.minutes()),
            partial.seconds.unwrap_or_else(|| duration.seconds()),
            partial
                .milliseconds
                .unwrap_or_else(|| duration.milliseconds()),
            partial
                .microseconds
                .unwrap_or_else(|| duration.microseconds()),
            partial
                .nanoseconds
                .unwrap_or_else(|| duration.nanoseconds()),
        )
        .map_err(temporal_error)?;
        self.create_duration_value(result)
    }

    fn eval_duration_round(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let duration = self.duration_receiver(receiver)?;
        let (options, relative_to) = self.duration_rounding_options(args.as_slice().first())?;
        let rounded = duration
            .round(options, relative_to)
            .map_err(temporal_error)?;
        self.create_duration_value(rounded)
    }

    fn eval_duration_total(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let duration = self.duration_receiver(receiver)?;
        let (unit, relative_to) = self.duration_total_options(args.as_slice().first())?;
        let total = duration.total(unit, relative_to).map_err(temporal_error)?;
        Ok(Value::Number(total.as_inner()))
    }

    fn duration_rounding_options(
        &mut self,
        value: Option<&Value>,
    ) -> Result<(RoundingOptions, Option<RelativeTo>)> {
        let Some(value) = value else {
            return Err(Error::type_error(DURATION_OPTIONS_ERROR));
        };
        if let Some(text) = value.string_text() {
            let mut options = RoundingOptions::default();
            options.smallest_unit = Some(Self::duration_unit(text)?);
            return Ok((options, None));
        }
        if !Self::is_object_value(value) {
            return Err(Error::type_error(DURATION_OPTIONS_ERROR));
        }
        let largest_unit = self.optional_duration_unit(value, "largestUnit")?;
        let relative_to = self.duration_relative_to_property(value)?;
        let increment_value = self.get_named(value, "roundingIncrement")?;
        let increment = if matches!(increment_value, Value::Undefined) {
            None
        } else {
            Some(
                RoundingIncrement::try_from(self.to_number(&increment_value)?)
                    .map_err(temporal_error)?,
            )
        };
        let rounding_mode = self.optional_rounding_mode(value, "roundingMode")?;
        let smallest_unit = self.optional_duration_unit(value, "smallestUnit")?;
        let mut options = RoundingOptions::default();
        options.largest_unit = largest_unit;
        options.smallest_unit = smallest_unit;
        options.rounding_mode = rounding_mode;
        options.increment = increment;
        Ok((options, relative_to))
    }

    fn duration_total_options(
        &mut self,
        value: Option<&Value>,
    ) -> Result<(Unit, Option<RelativeTo>)> {
        let Some(value) = value else {
            return Err(Error::type_error(DURATION_OPTIONS_ERROR));
        };
        if let Some(text) = value.string_text() {
            return Ok((Self::duration_unit(text)?, None));
        }
        if !Self::is_object_value(value) {
            return Err(Error::type_error(DURATION_OPTIONS_ERROR));
        }
        let relative_to = self.duration_relative_to_property(value)?;
        let unit = self.get_named(value, "unit")?;
        let text = self.to_string(&unit)?;
        Ok((Self::duration_unit(&text)?, relative_to))
    }

    fn duration_relative_to_option(
        &mut self,
        options: Option<&Value>,
    ) -> Result<Option<RelativeTo>> {
        let Some(options) = options else {
            return Ok(None);
        };
        if matches!(options, Value::Undefined) {
            return Ok(None);
        }
        if !Self::is_object_value(options) {
            return Err(Error::type_error(
                "Temporal.Duration options must be an object",
            ));
        }
        self.duration_relative_to_property(options)
    }

    fn duration_relative_to_property(&mut self, options: &Value) -> Result<Option<RelativeTo>> {
        let value = self.get_named(options, "relativeTo")?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        if let Value::Object(id) = &value {
            let relative = match self.objects.temporal_value(*id)? {
                Some(TemporalValue::PlainDate(date)) => Some(RelativeTo::from(date.clone())),
                Some(TemporalValue::PlainDateTime(date_time)) => {
                    Some(RelativeTo::from(date_time.to_plain_date()))
                }
                Some(TemporalValue::ZonedDateTime(zoned)) => Some(RelativeTo::from(zoned.clone())),
                _ => None,
            };
            if relative.is_some() {
                return Ok(relative);
            }
        }
        if Self::is_object_value(&value) {
            return self.duration_relative_to_property_bag(&value).map(Some);
        }
        let Some(text) = value.string_text() else {
            return Err(Error::type_error(
                "Temporal.Duration relativeTo must be a string or Temporal object",
            ));
        };
        RelativeTo::try_from_str(text)
            .map(Some)
            .map_err(temporal_error)
    }

    fn duration_relative_to_property_bag(&mut self, value: &Value) -> Result<RelativeTo> {
        let calendar_value = self.get_named(value, "calendar")?;
        let calendar = self.duration_relative_calendar(&calendar_value)?;
        let day = self.required_relative_i64(value, "day")?;
        let hour = self.optional_relative_i64(value, "hour")?;
        let microsecond = self.optional_relative_i64(value, "microsecond")?;
        let millisecond = self.optional_relative_i64(value, "millisecond")?;
        let minute = self.optional_relative_i64(value, "minute")?;
        let month = self.optional_relative_i64(value, "month")?;
        let month_code_value = self.get_named(value, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.to_string(&month_code_value)?)
        };
        let nanosecond = self.optional_relative_i64(value, "nanosecond")?;
        let offset_value = self.get_named(value, "offset")?;
        let offset = if matches!(offset_value, Value::Undefined) {
            None
        } else if let Some(text) = offset_value.string_text() {
            Some(text.to_owned())
        } else if Self::is_object_value(&offset_value) {
            Some(self.to_string(&offset_value)?)
        } else {
            return Err(Error::type_error(
                "Temporal relativeTo offset must be a string",
            ));
        };
        let second = self.optional_relative_i64(value, "second")?;
        let time_zone_value = self.get_named(value, "timeZone")?;
        let time_zone = Self::relative_string_property(&time_zone_value, "timeZone")?;
        let year = self.required_relative_i64(value, "year")?;

        let month = Self::resolve_relative_month(month, month_code.as_deref())?;
        let year = year.to_i32().ok_or_else(|| {
            Error::exception(ErrorName::RangeError, "relativeTo year is out of range")
        })?;
        let month = month.to_u8().ok_or_else(|| {
            Error::exception(ErrorName::RangeError, "relativeTo month is out of range")
        })?;
        let day = day.to_u8().ok_or_else(|| {
            Error::exception(ErrorName::RangeError, "relativeTo day is out of range")
        })?;
        let date = PlainDate::try_new(year, month, day, calendar).map_err(temporal_error)?;
        let Some(time_zone) = time_zone else {
            return Ok(RelativeTo::from(date));
        };

        let hour = Self::relative_time_component(hour, "hour")?;
        let minute = Self::relative_time_component(minute, "minute")?;
        let second = Self::relative_second_component(second)?;
        let millisecond = Self::relative_subsecond_component(millisecond, "millisecond")?;
        let microsecond = Self::relative_subsecond_component(microsecond, "microsecond")?;
        let nanosecond = Self::relative_subsecond_component(nanosecond, "nanosecond")?;
        let time_zone = temporal_rs::TimeZone::try_from_str(&time_zone)
            .and_then(|zone| zone.identifier())
            .map_err(temporal_error)?;
        let date_text = date.to_ixdtf_string(DisplayCalendar::Never);
        let offset = offset.unwrap_or_default();
        let calendar = date.calendar().identifier();
        let text = format!(
            "{date_text}T{hour:02}:{minute:02}:{second:02}.{millisecond:03}{microsecond:03}{nanosecond:03}{offset}[{time_zone}][u-ca={calendar}]"
        );
        RelativeTo::try_from_str(&text).map_err(temporal_error)
    }

    fn duration_relative_calendar(&self, value: &Value) -> Result<Calendar> {
        if matches!(value, Value::Undefined) {
            return Ok(Calendar::default());
        }
        if let Value::Object(id) = value {
            match self.objects.temporal_value(*id)? {
                Some(TemporalValue::PlainDate(date)) => return Ok(date.calendar().clone()),
                Some(TemporalValue::PlainDateTime(date_time)) => {
                    return Ok(date_time.calendar().clone());
                }
                Some(TemporalValue::PlainMonthDay(month_day)) => {
                    return Ok(month_day.calendar().clone());
                }
                Some(TemporalValue::PlainYearMonth(year_month)) => {
                    return Ok(year_month.calendar().clone());
                }
                Some(TemporalValue::ZonedDateTime(zoned)) => {
                    return Ok(zoned.calendar().clone());
                }
                _ => {}
            }
        }
        let Some(text) = value.string_text() else {
            return Err(Error::type_error(
                "Temporal relativeTo calendar must be a string or Temporal object",
            ));
        };
        Calendar::try_from_utf8(text.as_bytes()).map_err(temporal_error)
    }

    fn optional_relative_i64(&mut self, object: &Value, name: &str) -> Result<Option<i64>> {
        let value = self.get_named(object, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        self.duration_i64_argument(Some(&value)).map(Some)
    }

    fn required_relative_i64(&mut self, object: &Value, name: &str) -> Result<i64> {
        self.optional_relative_i64(object, name)?.ok_or_else(|| {
            Error::type_error(format!("Temporal relativeTo requires a {name} property"))
        })
    }

    fn relative_string_property(value: &Value, name: &str) -> Result<Option<String>> {
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        value
            .string_text()
            .map(str::to_owned)
            .map(Some)
            .ok_or_else(|| {
                Error::type_error(format!("Temporal relativeTo {name} must be a string"))
            })
    }

    fn resolve_relative_month(month: Option<i64>, month_code: Option<&str>) -> Result<i64> {
        let code_month = month_code
            .map(|code| {
                code.strip_prefix('M')
                    .and_then(|digits| digits.parse::<i64>().ok())
                    .filter(|value| (1..=12).contains(value))
                    .ok_or_else(|| {
                        Error::exception(ErrorName::RangeError, "Invalid relativeTo monthCode")
                    })
            })
            .transpose()?;
        match (month, code_month) {
            (Some(month), Some(code)) if month != code => Err(Error::exception(
                ErrorName::RangeError,
                "relativeTo month and monthCode do not agree",
            )),
            (Some(month), _) => Ok(month),
            (None, Some(code)) => Ok(code),
            (None, None) => Err(Error::type_error(
                "Temporal relativeTo requires month or monthCode",
            )),
        }
    }

    fn relative_time_component(value: Option<i64>, name: &str) -> Result<u8> {
        let value = value.unwrap_or_default();
        let converted = value.to_u8().filter(|value| *value <= 23);
        if name != "hour" {
            return value.to_u8().filter(|value| *value <= 59).ok_or_else(|| {
                Error::exception(
                    ErrorName::RangeError,
                    format!("relativeTo {name} is invalid"),
                )
            });
        }
        converted.ok_or_else(|| {
            Error::exception(
                ErrorName::RangeError,
                format!("relativeTo {name} is invalid"),
            )
        })
    }

    fn relative_second_component(value: Option<i64>) -> Result<u8> {
        value
            .unwrap_or_default()
            .to_u8()
            .filter(|value| *value <= 60)
            .ok_or_else(|| Error::exception(ErrorName::RangeError, "relativeTo second is invalid"))
    }

    fn relative_subsecond_component(value: Option<i64>, name: &str) -> Result<u16> {
        value
            .unwrap_or_default()
            .to_u16()
            .filter(|value| *value <= 999)
            .ok_or_else(|| {
                Error::exception(
                    ErrorName::RangeError,
                    format!("relativeTo {name} is invalid"),
                )
            })
    }

    fn optional_duration_unit(&mut self, object: &Value, name: &str) -> Result<Option<Unit>> {
        let value = self.get_named(object, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        let text = self.to_string(&value)?;
        Self::duration_unit(&text).map(Some)
    }

    fn optional_rounding_mode(
        &mut self,
        object: &Value,
        name: &str,
    ) -> Result<Option<RoundingMode>> {
        let value = self.get_named(object, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        let text = self.to_string(&value)?;
        RoundingMode::from_str(&text)
            .map(Some)
            .map_err(temporal_error)
    }

    fn duration_unit(text: &str) -> Result<Unit> {
        Unit::from_str(text).map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                format!("Invalid Temporal unit: {text}"),
            )
        })
    }

    fn eval_duration_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let options = self.duration_to_string_options(args.as_slice().first())?;
        let duration = self.duration_receiver(receiver)?;
        let text = duration
            .as_temporal_string(options)
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    fn duration_default_string(&mut self, receiver: &Value) -> Result<Value> {
        let duration = self.duration_receiver(receiver)?;
        let text = duration
            .as_temporal_string(ToStringRoundingOptions::default())
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    pub(super) fn duration_to_string_options(
        &mut self,
        value: Option<&Value>,
    ) -> Result<ToStringRoundingOptions> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(ToStringRoundingOptions::default());
        };
        if !Self::is_object_value(value) {
            return Err(Error::type_error(DURATION_OPTIONS_ERROR));
        }
        let fractional = self.get_named(value, "fractionalSecondDigits")?;
        let precision =
            if matches!(fractional, Value::Undefined) || fractional.string_text() == Some("auto") {
                Precision::Auto
            } else if let Value::Number(number) = fractional {
                let digits = number.floor();
                if !digits.is_finite() || !(0.0..=9.0).contains(&digits) {
                    return Err(Error::exception(
                        ErrorName::RangeError,
                        "fractionalSecondDigits must be from 0 through 9",
                    ));
                }
                Precision::Digit(digits.to_u8().ok_or_else(|| {
                    Error::exception(
                        ErrorName::RangeError,
                        "fractionalSecondDigits is out of range",
                    )
                })?)
            } else {
                let text = self.to_string(&fractional)?;
                if text == "auto" {
                    Precision::Auto
                } else {
                    return Err(Error::exception(
                        ErrorName::RangeError,
                        "fractionalSecondDigits must be 'auto' or a number",
                    ));
                }
            };
        let rounding_mode = self.optional_rounding_mode(value, "roundingMode")?;
        let smallest_unit = self.optional_duration_unit(value, "smallestUnit")?;
        Ok(ToStringRoundingOptions {
            precision,
            smallest_unit,
            rounding_mode,
        })
    }

    const fn is_object_value(value: &Value) -> bool {
        matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        )
    }
}
