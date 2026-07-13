use std::cmp::Ordering;

use num_traits::ToPrimitive;
use temporal_rs::{
    PlainDate, TimeZone,
    options::{DifferenceSettings, DisplayCalendar},
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::{ErrorName, Value},
};

use super::temporal_error;

impl Context {
    pub(super) fn eval_plain_date_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if let Some(result) = self.eval_plain_date_accessor_kind(kind, receiver) {
            return result;
        }
        match kind {
            TemporalFunctionKind::PlainDateConstructor => Err(Error::type_error(
                "Temporal.PlainDate constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainDateFrom => {
                let date = self.plain_date_from_value(args.as_slice().first())?;
                self.create_plain_date_value(date)
            }
            TemporalFunctionKind::PlainDateCompare => self.eval_plain_date_compare(args),
            TemporalFunctionKind::PlainDatePrototypeWith => {
                self.eval_plain_date_with(args, receiver)
            }
            TemporalFunctionKind::PlainDatePrototypeWithCalendar => {
                self.eval_plain_date_with_calendar(args, receiver)
            }
            TemporalFunctionKind::PlainDatePrototypeAdd => {
                self.eval_plain_date_add_subtract(args, receiver, false)
            }
            TemporalFunctionKind::PlainDatePrototypeSubtract => {
                self.eval_plain_date_add_subtract(args, receiver, true)
            }
            TemporalFunctionKind::PlainDatePrototypeUntil => {
                self.eval_plain_date_difference(args, receiver, false)
            }
            TemporalFunctionKind::PlainDatePrototypeSince => {
                self.eval_plain_date_difference(args, receiver, true)
            }
            TemporalFunctionKind::PlainDatePrototypeEquals => {
                self.eval_plain_date_equals(args, receiver)
            }
            TemporalFunctionKind::PlainDatePrototypeToPlainDateTime => {
                self.eval_plain_date_to_date_time(args, receiver)
            }
            TemporalFunctionKind::PlainDatePrototypeToZonedDateTime => {
                self.eval_plain_date_to_zoned_date_time(args, receiver)
            }
            TemporalFunctionKind::PlainDatePrototypeToPlainYearMonth => {
                let result = self
                    .plain_date_receiver(receiver)?
                    .to_plain_year_month()
                    .map_err(temporal_error)?;
                self.create_temporal_calendar_value(
                    TemporalValue::PlainYearMonth(result),
                    TemporalFunctionKind::PlainYearMonthConstructor,
                )
            }
            TemporalFunctionKind::PlainDatePrototypeToPlainMonthDay => {
                let result = self
                    .plain_date_receiver(receiver)?
                    .to_plain_month_day()
                    .map_err(temporal_error)?;
                self.create_temporal_calendar_value(
                    TemporalValue::PlainMonthDay(result),
                    TemporalFunctionKind::PlainMonthDayConstructor,
                )
            }
            TemporalFunctionKind::PlainDatePrototypeToString
            | TemporalFunctionKind::PlainDatePrototypeToJson
            | TemporalFunctionKind::PlainDatePrototypeToLocaleString => {
                let date = self.plain_date_receiver(receiver)?;
                self.heap_string_value(&date.to_ixdtf_string(DisplayCalendar::Auto))
            }
            TemporalFunctionKind::PlainDatePrototypeValueOf => Err(Error::type_error(
                "Temporal.PlainDate cannot be converted to a primitive",
            )),
            _ => Err(Error::runtime("PlainDate function kind was not handled")),
        }
    }

    fn eval_plain_date_accessor_kind(
        &mut self,
        kind: TemporalFunctionKind,
        receiver: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            TemporalFunctionKind::PlainDatePrototypeYear => {
                Some(self.plain_date_numeric(receiver, PlainDate::year))
            }
            TemporalFunctionKind::PlainDatePrototypeMonth => {
                Some(self.plain_date_numeric(receiver, PlainDate::month))
            }
            TemporalFunctionKind::PlainDatePrototypeDay => {
                Some(self.plain_date_numeric(receiver, PlainDate::day))
            }
            TemporalFunctionKind::PlainDatePrototypeMonthCode => Some(
                self.plain_date_receiver(receiver)
                    .and_then(|date| self.heap_string_value(date.month_code().as_str())),
            ),
            TemporalFunctionKind::PlainDatePrototypeCalendarId => Some(
                self.plain_date_receiver(receiver)
                    .and_then(|date| self.heap_string_value(date.calendar().identifier())),
            ),
            TemporalFunctionKind::PlainDatePrototypeEra => {
                Some(self.plain_date_receiver(receiver).and_then(|date| {
                    date.era().map_or(Ok(Value::Undefined), |era| {
                        self.heap_string_value(era.as_str())
                    })
                }))
            }
            TemporalFunctionKind::PlainDatePrototypeEraYear => {
                Some(self.plain_date_optional_numeric(receiver, PlainDate::era_year))
            }
            TemporalFunctionKind::PlainDatePrototypeDayOfWeek => {
                Some(self.plain_date_numeric(receiver, PlainDate::day_of_week))
            }
            TemporalFunctionKind::PlainDatePrototypeDayOfYear => {
                Some(self.plain_date_numeric(receiver, PlainDate::day_of_year))
            }
            TemporalFunctionKind::PlainDatePrototypeWeekOfYear => {
                Some(self.plain_date_optional_numeric(receiver, PlainDate::week_of_year))
            }
            TemporalFunctionKind::PlainDatePrototypeYearOfWeek => {
                Some(self.plain_date_optional_numeric(receiver, PlainDate::year_of_week))
            }
            TemporalFunctionKind::PlainDatePrototypeDaysInWeek => {
                Some(self.plain_date_numeric(receiver, PlainDate::days_in_week))
            }
            TemporalFunctionKind::PlainDatePrototypeDaysInMonth => {
                Some(self.plain_date_numeric(receiver, PlainDate::days_in_month))
            }
            TemporalFunctionKind::PlainDatePrototypeDaysInYear => {
                Some(self.plain_date_numeric(receiver, PlainDate::days_in_year))
            }
            TemporalFunctionKind::PlainDatePrototypeMonthsInYear => {
                Some(self.plain_date_numeric(receiver, PlainDate::months_in_year))
            }
            TemporalFunctionKind::PlainDatePrototypeInLeapYear => Some(
                self.plain_date_receiver(receiver)
                    .map(|date| Value::Bool(date.in_leap_year())),
            ),
            _ => None,
        }
    }

    fn plain_date_from_value(&mut self, value: Option<&Value>) -> Result<PlainDate> {
        let Some(value) = value else {
            return Err(Error::type_error("Temporal.PlainDate requires an argument"));
        };
        if let Ok(date) = self.plain_date_receiver(value) {
            return Ok(date);
        }
        if let Some(text) = value.string_text() {
            return PlainDate::from_utf8(text.as_bytes()).map_err(temporal_error);
        }
        let Value::Object(_) = value else {
            return Err(Error::type_error(
                "PlainDate input must be a string or object",
            ));
        };
        let calendar_value = self.get_named(value, "calendar")?;
        let calendar = self.temporal_calendar(Some(&calendar_value))?;
        let day = self.plain_date_required_i64(value, "day")?;
        let month = self.plain_date_optional_i64(value, "month")?;
        let month_code_value = self.get_named(value, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.to_string(&month_code_value)?)
        };
        let year = self.plain_date_required_i64(value, "year")?;
        let month = Self::plain_date_month(month, month_code.as_deref())?;
        PlainDate::try_new(
            year.to_i32()
                .ok_or_else(|| Self::plain_date_range("year"))?,
            month
                .to_u8()
                .ok_or_else(|| Self::plain_date_range("month"))?,
            day.to_u8().ok_or_else(|| Self::plain_date_range("day"))?,
            calendar,
        )
        .map_err(temporal_error)
    }

    fn eval_plain_date_compare(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let one = self.plain_date_from_value(values.first())?;
        let two = self.plain_date_from_value(values.get(1))?;
        let result = match one.compare_iso(&two) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        };
        Ok(Value::Number(result))
    }

    fn eval_plain_date_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let Some(Value::Object(fields)) = args.as_slice().first() else {
            return Err(Error::type_error("PlainDate.with requires an object"));
        };
        let object = Value::Object(*fields);
        let year = self.plain_date_optional_i64(&object, "year")?;
        let month = self.plain_date_optional_i64(&object, "month")?;
        let day = self.plain_date_optional_i64(&object, "day")?;
        let result = PlainDate::try_new(
            year.and_then(|value| value.to_i32())
                .unwrap_or_else(|| date.year()),
            month
                .and_then(|value| value.to_u8())
                .unwrap_or_else(|| date.month()),
            day.and_then(|value| value.to_u8())
                .unwrap_or_else(|| date.day()),
            date.calendar().clone(),
        )
        .map_err(temporal_error)?;
        self.create_plain_date_value(result)
    }

    fn eval_plain_date_with_calendar(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let calendar = self.temporal_calendar(args.as_slice().first())?;
        self.create_plain_date_value(date.with_calendar(calendar))
    }

    fn eval_plain_date_add_subtract(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        subtract: bool,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let duration = self.duration_from_value(args.as_slice().first())?;
        let result = if subtract {
            date.subtract(&duration, None)
        } else {
            date.add(&duration, None)
        }
        .map_err(temporal_error)?;
        self.create_plain_date_value(result)
    }

    fn eval_plain_date_difference(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        since: bool,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let other = self.plain_date_from_value(args.as_slice().first())?;
        let result = if since {
            date.since(&other, DifferenceSettings::default())
        } else {
            date.until(&other, DifferenceSettings::default())
        }
        .map_err(temporal_error)?;
        self.create_duration_value(result)
    }

    fn eval_plain_date_equals(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let other = self.plain_date_from_value(args.as_slice().first())?;
        Ok(Value::Bool(
            date.compare_iso(&other) == Ordering::Equal
                && date.calendar().identifier() == other.calendar().identifier(),
        ))
    }

    fn eval_plain_date_to_date_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let time = match args.as_slice().first() {
            Some(Value::Object(id)) => match self.objects.temporal_value(*id)? {
                Some(TemporalValue::PlainTime(time)) => Some(*time),
                _ => return Err(Error::type_error("toPlainDateTime requires a PlainTime")),
            },
            Some(Value::Undefined) | None => None,
            _ => return Err(Error::type_error("toPlainDateTime requires a PlainTime")),
        };
        let result = date.to_plain_date_time(time).map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::PlainDateTime(result),
            TemporalFunctionKind::PlainDateTimeConstructor,
        )
    }

    fn eval_plain_date_to_zoned_date_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let Some(value) = args.as_slice().first() else {
            return Err(Error::type_error("toZonedDateTime requires a time zone"));
        };
        let zone_text = if let Some(text) = value.string_text() {
            text.to_owned()
        } else if let Value::Object(_) = value {
            let zone = self.get_named(value, "timeZone")?;
            self.to_string(&zone)?
        } else {
            return Err(Error::type_error("toZonedDateTime requires a time zone"));
        };
        let zone = TimeZone::try_from_str(&zone_text).map_err(temporal_error)?;
        let result = date
            .to_zoned_date_time(zone, None)
            .map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::ZonedDateTime(result),
            TemporalFunctionKind::ZonedDateTimeConstructor,
        )
    }

    fn plain_date_numeric<T>(&self, receiver: &Value, getter: fn(&PlainDate) -> T) -> Result<Value>
    where
        T: ToPrimitive,
    {
        let date = self.plain_date_receiver(receiver)?;
        let value = getter(&date)
            .to_f64()
            .ok_or_else(|| Error::runtime("PlainDate field cannot become Number"))?;
        Ok(Value::Number(value))
    }

    fn plain_date_optional_numeric<T>(
        &self,
        receiver: &Value,
        getter: fn(&PlainDate) -> Option<T>,
    ) -> Result<Value>
    where
        T: ToPrimitive,
    {
        let date = self.plain_date_receiver(receiver)?;
        let Some(value) = getter(&date) else {
            return Ok(Value::Undefined);
        };
        value
            .to_f64()
            .map(Value::Number)
            .ok_or_else(|| Error::runtime("PlainDate field cannot become Number"))
    }

    fn plain_date_optional_i64(&mut self, object: &Value, name: &str) -> Result<Option<i64>> {
        let value = self.get_named(object, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        let number = self.to_number(&value)?;
        if !number.is_finite() || number.fract() != 0.0 {
            return Err(Self::plain_date_range(name));
        }
        number
            .to_i64()
            .map(Some)
            .ok_or_else(|| Self::plain_date_range(name))
    }

    fn plain_date_required_i64(&mut self, object: &Value, name: &str) -> Result<i64> {
        self.plain_date_optional_i64(object, name)?
            .ok_or_else(|| Error::type_error(format!("PlainDate requires {name}")))
    }

    fn plain_date_month(month: Option<i64>, month_code: Option<&str>) -> Result<i64> {
        let from_code = month_code
            .map(|code| {
                code.strip_prefix('M')
                    .and_then(|digits| digits.parse::<i64>().ok())
                    .filter(|value| (1..=12).contains(value))
                    .ok_or_else(|| Self::plain_date_range("monthCode"))
            })
            .transpose()?;
        match (month, from_code) {
            (Some(month), Some(code)) if month != code => Err(Self::plain_date_range("monthCode")),
            (Some(month), _) => Ok(month),
            (None, Some(code)) => Ok(code),
            (None, None) => Err(Error::type_error("PlainDate requires month or monthCode")),
        }
    }

    fn plain_date_range(field: &str) -> Error {
        Error::exception(
            ErrorName::RangeError,
            format!("PlainDate {field} is invalid"),
        )
    }
}
