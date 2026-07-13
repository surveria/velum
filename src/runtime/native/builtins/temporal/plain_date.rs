use std::{cmp::Ordering, str::FromStr};

use num_traits::ToPrimitive;
use temporal_rs::{
    Calendar, MonthCode, PlainDate, PlainTime, TimeZone,
    fields::CalendarFields,
    options::{
        DifferenceSettings, DisplayCalendar, Overflow, RoundingIncrement, RoundingMode, Unit,
    },
    partial::{PartialDate, PartialTime},
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::{ErrorName, Value},
};

use super::temporal_error;

enum PlainDateInput {
    Resolved(PlainDate),
    String(String),
    Fields(PlainDateFields),
}

struct PlainDateFields {
    calendar: Calendar,
    day: i64,
    month: Option<i64>,
    month_code: Option<MonthCode>,
    year: i64,
}

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
                let values = args.as_slice();
                let input = self.prepare_plain_date_input(values.first())?;
                if let PlainDateInput::String(text) = input {
                    let date = PlainDate::from_utf8(text.as_bytes()).map_err(temporal_error)?;
                    self.plain_date_overflow_option(values.get(1))?;
                    return self.create_plain_date_value(date);
                }
                let overflow = self.plain_date_overflow_option(values.get(1))?;
                let date = Self::resolve_plain_date_input(input, overflow)?;
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
            TemporalFunctionKind::PlainDatePrototypeToString => {
                let display = self.plain_date_display_calendar(args.as_slice().first())?;
                let date = self.plain_date_receiver(receiver)?;
                self.heap_string_value(&date.to_ixdtf_string(display))
            }
            TemporalFunctionKind::PlainDatePrototypeToJson => {
                let date = self.plain_date_receiver(receiver)?;
                self.heap_string_value(&date.to_ixdtf_string(DisplayCalendar::Auto))
            }
            TemporalFunctionKind::PlainDatePrototypeToLocaleString => {
                self.format_temporal_locale_string(receiver, args)
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
        let input = self.prepare_plain_date_input(value)?;
        Self::resolve_plain_date_input(input, Overflow::Constrain)
    }

    fn prepare_plain_date_input(&mut self, value: Option<&Value>) -> Result<PlainDateInput> {
        let Some(value) = value else {
            return Err(Error::type_error("Temporal.PlainDate requires an argument"));
        };
        if let Value::Object(id) = value {
            match self.objects.temporal_value(*id)? {
                Some(TemporalValue::PlainDate(date)) => {
                    return Ok(PlainDateInput::Resolved(date.clone()));
                }
                Some(TemporalValue::PlainDateTime(date_time)) => {
                    return Ok(PlainDateInput::Resolved(date_time.to_plain_date()));
                }
                Some(TemporalValue::ZonedDateTime(zoned)) => {
                    return Ok(PlainDateInput::Resolved(zoned.to_plain_date()));
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
            return Ok(PlainDateInput::String(text.to_owned()));
        }
        let Value::Object(_) = value else {
            return Err(Error::type_error(
                "PlainDate input must be a string or object",
            ));
        };
        let calendar_value = self.get_named(value, "calendar")?;
        let calendar = self.temporal_calendar(Some(&calendar_value))?;
        let day = self.plain_date_required_i64(value, "day")?;
        if !matches!(calendar.identifier(), "iso8601" | "chinese" | "dangi") {
            self.validate_plain_date_era_pair(value)?;
        }
        let month = self.plain_date_optional_i64(value, "month")?;
        let month_code_value = self.get_named(value, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_date_month_code(&month_code_value)?)
        };
        let year = self.plain_date_required_i64(value, "year")?;
        Ok(PlainDateInput::Fields(PlainDateFields {
            calendar,
            day,
            month,
            month_code,
            year,
        }))
    }

    fn resolve_plain_date_input(input: PlainDateInput, overflow: Overflow) -> Result<PlainDate> {
        match input {
            PlainDateInput::Resolved(date) => Ok(date),
            PlainDateInput::String(text) => {
                PlainDate::from_utf8(text.as_bytes()).map_err(temporal_error)
            }
            PlainDateInput::Fields(fields) => {
                let year = fields
                    .year
                    .to_i32()
                    .ok_or_else(|| Self::plain_date_range("year"))?;
                let month = fields
                    .month
                    .map(|value| Self::plain_date_u8_field(value, "month", overflow))
                    .transpose()?;
                let day = Self::plain_date_u8_field(fields.day, "day", overflow)?;
                let partial = PartialDate::new()
                    .with_year(Some(year))
                    .with_month(month)
                    .with_month_code(fields.month_code)
                    .with_day(Some(day))
                    .with_calendar(fields.calendar);
                PlainDate::from_partial(partial, Some(overflow)).map_err(temporal_error)
            }
        }
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
        let values = args.as_slice();
        let Some(Value::Object(fields)) = values.first() else {
            return Err(Error::type_error("PlainDate.with requires an object"));
        };
        let object = Value::Object(*fields);
        for name in ["calendar", "timeZone"] {
            if !matches!(self.get_named(&object, name)?, Value::Undefined) {
                return Err(Error::type_error(format!(
                    "PlainDate.with does not accept {name}"
                )));
            }
        }
        let day = self.plain_date_optional_i64(&object, "day")?;
        let month = self.plain_date_optional_i64(&object, "month")?;
        let month_code_value = self.get_named(&object, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_date_month_code(&month_code_value)?)
        };
        let year = self.plain_date_optional_i64(&object, "year")?;
        if day.is_some_and(|value| value <= 0) {
            return Err(Self::plain_date_range("day"));
        }
        if month.is_some_and(|value| value <= 0) {
            return Err(Self::plain_date_range("month"));
        }
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let fields = CalendarFields::new()
            .with_optional_year(
                year.map(|value| value.to_i32().ok_or_else(|| Self::plain_date_range("year")))
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
        let result = date.with(fields, Some(overflow)).map_err(temporal_error)?;
        self.create_plain_date_value(result)
    }

    fn eval_plain_date_with_calendar(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let Some(calendar_value) = args.as_slice().first() else {
            return Err(Error::type_error(
                "PlainDate.withCalendar requires an argument",
            ));
        };
        if matches!(calendar_value, Value::Undefined) {
            return Err(Error::type_error(
                "PlainDate.withCalendar requires an argument",
            ));
        }
        let calendar = self.temporal_calendar(Some(calendar_value))?;
        self.create_plain_date_value(date.with_calendar(calendar))
    }

    fn eval_plain_date_add_subtract(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        subtract: bool,
    ) -> Result<Value> {
        let date = self.plain_date_receiver(receiver)?;
        let values = args.as_slice();
        let duration = self.duration_from_value(values.first())?;
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let result = if subtract {
            date.subtract(&duration, Some(overflow))
        } else {
            date.add(&duration, Some(overflow))
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
        let values = args.as_slice();
        let other = self.plain_date_from_value(values.first())?;
        let settings = self.plain_date_difference_settings(values.get(1))?;
        let result = if since {
            date.since(&other, settings)
        } else {
            date.until(&other, settings)
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
            Some(Value::Undefined) | None => None,
            Some(value) => Some(self.plain_time_from_value(value)?),
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
        let (zone_value, time_value) = if value.string_text().is_some() {
            (value.clone(), Value::Undefined)
        } else if let Value::Object(_) = value {
            let zone = self.get_named(value, "timeZone")?;
            let time = self.get_named(value, "plainTime")?;
            (zone, time)
        } else {
            return Err(Error::type_error("toZonedDateTime requires a time zone"));
        };
        let Some(zone_text) = zone_value.string_text() else {
            return Err(Error::type_error("Temporal time zone must be a string"));
        };
        let zone = TimeZone::try_from_str(zone_text).map_err(temporal_error)?;
        let time = if matches!(time_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_time_from_value(&time_value)?)
        };
        let result = date
            .to_zoned_date_time(zone, time)
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

    pub(super) fn plain_date_optional_i64(
        &mut self,
        object: &Value,
        name: &str,
    ) -> Result<Option<i64>> {
        let value = self.get_named(object, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        let number = self.to_number(&value)?;
        if !number.is_finite() {
            return Err(Self::plain_date_range(name));
        }
        number
            .trunc()
            .to_i64()
            .map(Some)
            .ok_or_else(|| Self::plain_date_range(name))
    }

    pub(super) fn plain_date_required_i64(&mut self, object: &Value, name: &str) -> Result<i64> {
        self.plain_date_optional_i64(object, name)?
            .ok_or_else(|| Error::type_error(format!("PlainDate requires {name}")))
    }

    pub(super) fn plain_date_u8_field(value: i64, name: &str, overflow: Overflow) -> Result<u8> {
        if value <= 0 {
            return Err(Self::plain_date_range(name));
        }
        let normalized = match overflow {
            Overflow::Constrain => value.clamp(1, i64::from(u8::MAX)),
            Overflow::Reject => value,
        };
        normalized
            .to_u8()
            .ok_or_else(|| Self::plain_date_range(name))
    }

    pub(super) fn plain_date_month_code(&mut self, value: &Value) -> Result<MonthCode> {
        if value.string_text().is_none() && !Self::plain_date_object(value) {
            return Err(Error::type_error("PlainDate monthCode must be a string"));
        }
        let text = self.to_string(value)?;
        MonthCode::from_str(&text).map_err(|error| {
            if value.string_text().is_some() {
                temporal_error(error)
            } else {
                Error::type_error("PlainDate monthCode must convert to a valid string")
            }
        })
    }

    fn validate_plain_date_era_pair(&mut self, value: &Value) -> Result<()> {
        let era = self.get_named(value, "era")?;
        let era_year = self.get_named(value, "eraYear")?;
        let has_era = !matches!(era, Value::Undefined);
        let has_era_year = !matches!(era_year, Value::Undefined);
        if has_era != has_era_year {
            return Err(Error::type_error(
                "PlainDate era and eraYear must be provided together",
            ));
        }
        if has_era {
            self.to_string(&era)?;
            let number = self.to_number(&era_year)?;
            if !number.is_finite() {
                return Err(Self::plain_date_range("eraYear"));
            }
        }
        Ok(())
    }

    pub(super) fn plain_date_overflow_option(&mut self, value: Option<&Value>) -> Result<Overflow> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(Overflow::Constrain);
        };
        if !Self::plain_date_object(value) {
            return Err(Error::type_error("Temporal options must be an object"));
        }
        let overflow = self.get_named(value, "overflow")?;
        if matches!(overflow, Value::Undefined) {
            return Ok(Overflow::Constrain);
        }
        let text = self.to_string(&overflow)?;
        Overflow::from_str(&text).map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                format!("Invalid Temporal overflow: {text}"),
            )
        })
    }

    pub(super) fn plain_date_difference_settings(
        &mut self,
        value: Option<&Value>,
    ) -> Result<DifferenceSettings> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(DifferenceSettings::default());
        };
        if !Self::plain_date_object(value) {
            return Err(Error::type_error("Temporal options must be an object"));
        }
        let largest_unit = self.plain_date_unit_option(value, "largestUnit")?;
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
        let smallest_unit = self.plain_date_unit_option(value, "smallestUnit")?;
        let mut settings = DifferenceSettings::default();
        settings.largest_unit = largest_unit;
        settings.smallest_unit = smallest_unit;
        settings.rounding_mode = rounding_mode;
        settings.increment = increment;
        Ok(settings)
    }

    fn plain_date_unit_option(&mut self, value: &Value, name: &str) -> Result<Option<Unit>> {
        let option = self.get_named(value, name)?;
        if matches!(option, Value::Undefined) {
            return Ok(None);
        }
        let text = self.to_string(&option)?;
        Unit::from_str(&text).map(Some).map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                format!("Invalid Temporal unit: {text}"),
            )
        })
    }

    pub(super) fn plain_date_display_calendar(
        &mut self,
        value: Option<&Value>,
    ) -> Result<DisplayCalendar> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(DisplayCalendar::Auto);
        };
        if !Self::plain_date_object(value) {
            return Err(Error::type_error("Temporal options must be an object"));
        }
        let option = self.get_named(value, "calendarName")?;
        if matches!(option, Value::Undefined) {
            return Ok(DisplayCalendar::Auto);
        }
        let text = self.to_string(&option)?;
        DisplayCalendar::from_str(&text).map_err(temporal_error)
    }

    pub(super) fn plain_time_from_value(&mut self, value: &Value) -> Result<PlainTime> {
        if let Value::Object(id) = value {
            match self.objects.temporal_value(*id)? {
                Some(TemporalValue::PlainTime(time)) => return Ok(*time),
                Some(TemporalValue::PlainDateTime(date_time)) => {
                    return Ok(date_time.to_plain_time());
                }
                Some(TemporalValue::ZonedDateTime(zoned)) => return Ok(zoned.to_plain_time()),
                Some(
                    TemporalValue::Duration(_)
                    | TemporalValue::Instant(_)
                    | TemporalValue::PlainDate(_)
                    | TemporalValue::PlainMonthDay(_)
                    | TemporalValue::PlainYearMonth(_),
                )
                | None => {}
            }
        }
        if let Some(text) = value.string_text() {
            return PlainTime::from_utf8(text.as_bytes()).map_err(temporal_error);
        }
        if !Self::plain_date_object(value) {
            return Err(Error::type_error(
                "Temporal time must be a string or object",
            ));
        }
        let hour = self.plain_time_component(value, "hour", 23)?;
        let microsecond = self.plain_time_component(value, "microsecond", 999)?;
        let millisecond = self.plain_time_component(value, "millisecond", 999)?;
        let minute = self.plain_time_component(value, "minute", 59)?;
        let nanosecond = self.plain_time_component(value, "nanosecond", 999)?;
        let second = self.plain_time_component(value, "second", 59)?;
        let partial = PartialTime::new()
            .with_hour(hour.and_then(|value| value.to_u8()))
            .with_microsecond(microsecond)
            .with_millisecond(millisecond)
            .with_minute(minute.and_then(|value| value.to_u8()))
            .with_nanosecond(nanosecond)
            .with_second(second.and_then(|value| value.to_u8()));
        PlainTime::from_partial(partial, Some(Overflow::Constrain)).map_err(temporal_error)
    }

    fn plain_time_component(
        &mut self,
        value: &Value,
        name: &str,
        maximum: u16,
    ) -> Result<Option<u16>> {
        let field = self.get_named(value, name)?;
        if matches!(field, Value::Undefined) {
            return Ok(None);
        }
        let number = self.to_number(&field)?;
        if !number.is_finite() {
            return Err(Self::plain_date_range(name));
        }
        number
            .trunc()
            .clamp(0.0, f64::from(maximum))
            .to_u16()
            .map(Some)
            .ok_or_else(|| Self::plain_date_range(name))
    }

    const fn plain_date_object(value: &Value) -> bool {
        matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        )
    }

    fn plain_date_range(field: &str) -> Error {
        Error::exception(
            ErrorName::RangeError,
            format!("PlainDate {field} is invalid"),
        )
    }
}
