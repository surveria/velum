use std::str::FromStr;

use num_traits::ToPrimitive;
use temporal_rs::{
    fields::{CalendarFields, DateTimeFields},
    options::{RoundingIncrement, RoundingMode, RoundingOptions, Unit},
    partial::PartialTime,
};

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::{ErrorName, Value},
};

use super::temporal_error;

impl Context {
    pub(super) fn eval_plain_date_time_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let values = args.as_slice();
        let Some(Value::Object(id)) = values.first() else {
            return Err(Error::type_error("PlainDateTime.with requires an object"));
        };
        let object = Value::Object(*id);
        for name in ["calendar", "timeZone"] {
            if !matches!(self.get_named(&object, name)?, Value::Undefined) {
                return Err(Error::type_error(format!(
                    "PlainDateTime.with does not accept {name}"
                )));
            }
        }
        let day = self.plain_date_optional_i64(&object, "day")?;
        let hour = self.plain_date_optional_i64(&object, "hour")?;
        let microsecond = self.plain_date_optional_i64(&object, "microsecond")?;
        let millisecond = self.plain_date_optional_i64(&object, "millisecond")?;
        let minute = self.plain_date_optional_i64(&object, "minute")?;
        let month = self.plain_date_optional_i64(&object, "month")?;
        let month_code_value = self.get_named(&object, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_date_month_code(&month_code_value)?)
        };
        let nanosecond = self.plain_date_optional_i64(&object, "nanosecond")?;
        let second = self.plain_date_optional_i64(&object, "second")?;
        let year = self.plain_date_optional_i64(&object, "year")?;
        if day.is_some_and(|value| value <= 0) {
            return Err(Self::plain_date_time_range("day is invalid"));
        }
        if month.is_some_and(|value| value <= 0) {
            return Err(Self::plain_date_time_range("month is invalid"));
        }
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let calendar_fields = CalendarFields::new()
            .with_optional_year(
                year.map(|value| {
                    value
                        .to_i32()
                        .ok_or_else(|| Self::plain_date_time_range("year is invalid"))
                })
                .transpose()?,
            )
            .with_optional_month(
                month
                    .map(|value| Self::plain_date_u8_field(value, "month", overflow))
                    .transpose()?,
            )
            .with_optional_month_code(month_code)
            .with_optional_day(
                day.map(|value| Self::plain_date_u8_field(value, "day", overflow))
                    .transpose()?,
            );
        let time = PartialTime::new()
            .with_hour(Self::plain_date_time_u8_field(hour, "hour", 23, overflow)?)
            .with_microsecond(Self::plain_date_time_u16_field(
                microsecond,
                "microsecond",
                overflow,
            )?)
            .with_millisecond(Self::plain_date_time_u16_field(
                millisecond,
                "millisecond",
                overflow,
            )?)
            .with_minute(Self::plain_date_time_u8_field(
                minute, "minute", 59, overflow,
            )?)
            .with_nanosecond(Self::plain_date_time_u16_field(
                nanosecond,
                "nanosecond",
                overflow,
            )?)
            .with_second(Self::plain_date_time_u8_field(
                second, "second", 59, overflow,
            )?);
        let result = date_time
            .with(
                DateTimeFields {
                    calendar_fields,
                    time,
                },
                Some(overflow),
            )
            .map_err(temporal_error)?;
        self.create_plain_date_time_value(result)
    }

    pub(super) fn eval_plain_date_time_round(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date_time = self.plain_date_time_receiver(receiver)?;
        let options = self.plain_date_time_rounding_options(args.as_slice().first())?;
        let result = date_time.round(options).map_err(temporal_error)?;
        self.create_plain_date_time_value(result)
    }

    pub(super) fn eval_plain_date_time_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let value = args.as_slice().first();
        let display = self.plain_date_display_calendar(value)?;
        let options = self.duration_to_string_options(value)?;
        let date_time = self.plain_date_time_receiver(receiver)?;
        let text = date_time
            .to_ixdtf_string(options, display)
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    pub(super) fn plain_date_time_rounding_options(
        &mut self,
        value: Option<&Value>,
    ) -> Result<RoundingOptions> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Err(Error::type_error(
                "PlainDateTime.round requires an argument",
            ));
        };
        if let Some(text) = value.string_text() {
            let mut options = RoundingOptions::default();
            options.smallest_unit = Some(Self::plain_date_time_unit(text)?);
            return Ok(options);
        }
        let Value::Object(_) = value else {
            return Err(Error::type_error(
                "PlainDateTime.round options must be a string or object",
            ));
        };
        let increment_value = self.get_named(value, "roundingIncrement")?;
        let increment = if matches!(increment_value, Value::Undefined) {
            None
        } else {
            Some(
                RoundingIncrement::try_from(self.to_number(&increment_value)?)
                    .map_err(temporal_error)?,
            )
        };
        let rounding_mode_value = self.get_named(value, "roundingMode")?;
        let rounding_mode = if matches!(rounding_mode_value, Value::Undefined) {
            None
        } else {
            let text = self.to_string(&rounding_mode_value)?;
            Some(RoundingMode::from_str(&text).map_err(temporal_error)?)
        };
        let smallest_value = self.get_named(value, "smallestUnit")?;
        if matches!(smallest_value, Value::Undefined) {
            return Err(Error::exception(
                ErrorName::RangeError,
                "PlainDateTime.round requires smallestUnit",
            ));
        }
        let text = self.to_string(&smallest_value)?;
        let smallest_unit = Some(Self::plain_date_time_unit(&text)?);
        let mut options = RoundingOptions::default();
        options.smallest_unit = smallest_unit;
        options.rounding_mode = rounding_mode;
        options.increment = increment;
        Ok(options)
    }

    fn plain_date_time_unit(text: &str) -> Result<Unit> {
        Unit::from_str(text).map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                format!("Invalid Temporal unit: {text}"),
            )
        })
    }
}
