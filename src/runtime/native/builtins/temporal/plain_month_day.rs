use num_traits::ToPrimitive;
use temporal_rs::{
    PlainMonthDay, TinyAsciiStr,
    fields::CalendarFields,
    options::{DisplayCalendar, Overflow},
    partial::PartialDate,
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::{ErrorName, Value},
};

use super::temporal_error;

pub(super) const STATIC_METHODS: &[(&str, TemporalFunctionKind)] =
    &[("from", TemporalFunctionKind::PlainMonthDayFrom)];

pub(super) const ACCESSORS: &[(&str, TemporalFunctionKind)] = &[
    (
        "calendarId",
        TemporalFunctionKind::PlainMonthDayPrototypeCalendarId,
    ),
    (
        "monthCode",
        TemporalFunctionKind::PlainMonthDayPrototypeMonthCode,
    ),
    ("day", TemporalFunctionKind::PlainMonthDayPrototypeDay),
];

pub(super) const METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("with", TemporalFunctionKind::PlainMonthDayPrototypeWith),
    ("equals", TemporalFunctionKind::PlainMonthDayPrototypeEquals),
    (
        "toPlainDate",
        TemporalFunctionKind::PlainMonthDayPrototypeToPlainDate,
    ),
    (
        "toString",
        TemporalFunctionKind::PlainMonthDayPrototypeToString,
    ),
    (
        "toLocaleString",
        TemporalFunctionKind::PlainMonthDayPrototypeToLocaleString,
    ),
    ("toJSON", TemporalFunctionKind::PlainMonthDayPrototypeToJson),
    (
        "valueOf",
        TemporalFunctionKind::PlainMonthDayPrototypeValueOf,
    ),
];

impl Context {
    pub(super) fn eval_plain_month_day_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        match kind {
            TemporalFunctionKind::PlainMonthDayConstructor => Err(Error::type_error(
                "Temporal.PlainMonthDay constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainMonthDayFrom => self.eval_plain_month_day_from(args),
            TemporalFunctionKind::PlainMonthDayPrototypeCalendarId => {
                let value = self.plain_month_day_receiver(receiver)?;
                self.heap_string_value(value.calendar_id())
            }
            TemporalFunctionKind::PlainMonthDayPrototypeMonthCode => {
                let value = self.plain_month_day_receiver(receiver)?;
                self.heap_string_value(value.month_code().as_str())
            }
            TemporalFunctionKind::PlainMonthDayPrototypeDay => {
                let value = self.plain_month_day_receiver(receiver)?;
                Ok(Value::Number(f64::from(value.day())))
            }
            TemporalFunctionKind::PlainMonthDayPrototypeWith => {
                self.eval_plain_month_day_with(args, receiver)
            }
            TemporalFunctionKind::PlainMonthDayPrototypeEquals => {
                self.eval_plain_month_day_equals(args, receiver)
            }
            TemporalFunctionKind::PlainMonthDayPrototypeToPlainDate => {
                self.eval_plain_month_day_to_plain_date(args, receiver)
            }
            TemporalFunctionKind::PlainMonthDayPrototypeToString => {
                let display = self.plain_date_display_calendar(args.as_slice().first())?;
                self.plain_month_day_string(receiver, display)
            }
            TemporalFunctionKind::PlainMonthDayPrototypeToLocaleString => {
                self.format_temporal_locale_string(receiver, args)
            }
            TemporalFunctionKind::PlainMonthDayPrototypeToJson => {
                self.plain_month_day_string(receiver, DisplayCalendar::Auto)
            }
            TemporalFunctionKind::PlainMonthDayPrototypeValueOf => Err(Error::type_error(
                "Temporal.PlainMonthDay cannot be converted to a primitive",
            )),
            _ => Err(Error::runtime(
                "PlainMonthDay function kind was not handled",
            )),
        }
    }

    fn eval_plain_month_day_from(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let value = values
            .first()
            .ok_or_else(|| Error::type_error("PlainMonthDay.from requires an argument"))?;
        let month_day = if let Some(resolved) = self.plain_month_day_temporal(value)? {
            self.plain_date_overflow_option(values.get(1))?;
            resolved
        } else if let Some(text) = value.string_text() {
            let parsed = PlainMonthDay::from_utf8(text.as_bytes()).map_err(temporal_error)?;
            self.plain_date_overflow_option(values.get(1))?;
            parsed
        } else {
            self.plain_month_day_from_fields(value, values.get(1))?
        };
        self.create_plain_month_day_value(month_day)
    }

    fn plain_month_day_from_fields(
        &mut self,
        value: &Value,
        options: Option<&Value>,
    ) -> Result<PlainMonthDay> {
        let Value::Object(_) = value else {
            return Err(Error::type_error(
                "PlainMonthDay input must be a string or object",
            ));
        };
        let calendar_value = self.get_named(value, "calendar")?;
        let calendar = self.temporal_calendar(Some(&calendar_value))?;
        let day = self.plain_date_required_i64(value, "day")?;
        let (era, era_year) = self.temporal_calendar_era_fields(value, &calendar)?;
        let month = self.plain_date_optional_i64(value, "month")?;
        let month_code_value = self.get_named(value, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_date_month_code(&month_code_value)?)
        };
        let year = self.plain_date_optional_i64(value, "year")?;
        let overflow = self.plain_date_overflow_option(options)?;
        let fields =
            Self::plain_month_day_fields(day, era, era_year, month, month_code, year, overflow)?;
        PlainMonthDay::from_partial(
            PartialDate {
                calendar_fields: fields,
                calendar,
            },
            Some(overflow),
        )
        .map_err(temporal_error)
    }

    fn eval_plain_month_day_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let month_day = self.plain_month_day_receiver(receiver)?;
        let values = args.as_slice();
        let Some(Value::Object(id)) = values.first() else {
            return Err(Error::type_error("PlainMonthDay.with requires an object"));
        };
        if self.objects.temporal_value(*id)?.is_some() {
            return Err(Error::type_error(
                "PlainMonthDay.with does not accept a Temporal object",
            ));
        }
        let object = Value::Object(*id);
        for name in ["calendar", "timeZone"] {
            if !matches!(self.get_named(&object, name)?, Value::Undefined) {
                return Err(Error::type_error(format!(
                    "PlainMonthDay.with does not accept {name}"
                )));
            }
        }
        let day = self.plain_date_optional_i64(&object, "day")?;
        let (era, era_year) = self.temporal_calendar_era_fields(&object, month_day.calendar())?;
        let month = self.plain_date_optional_i64(&object, "month")?;
        let month_code_value = self.get_named(&object, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_date_month_code(&month_code_value)?)
        };
        let year = self.plain_date_optional_i64(&object, "year")?;
        if day.is_some_and(|value| value <= 0) {
            return Err(Self::plain_month_day_range("day"));
        }
        if month.is_some_and(|value| value <= 0) {
            return Err(Self::plain_month_day_range("month"));
        }
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let fields = Self::plain_month_day_optional_fields(
            day, era, era_year, month, month_code, year, overflow,
        )?;
        let result = month_day
            .with(fields, Some(overflow))
            .map_err(temporal_error)?;
        self.create_plain_month_day_value(result)
    }

    fn eval_plain_month_day_equals(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let one = self.plain_month_day_receiver(receiver)?;
        let other = args
            .as_slice()
            .first()
            .ok_or_else(|| Error::type_error("PlainMonthDay.equals requires an argument"))?;
        let two = if let Some(resolved) = self.plain_month_day_temporal(other)? {
            resolved
        } else if let Some(text) = other.string_text() {
            PlainMonthDay::from_utf8(text.as_bytes()).map_err(temporal_error)?
        } else {
            self.plain_month_day_from_fields(other, None)?
        };
        Ok(Value::Bool(one == two))
    }

    fn eval_plain_month_day_to_plain_date(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let month_day = self.plain_month_day_receiver(receiver)?;
        let Some(value @ Value::Object(_)) = args.as_slice().first() else {
            return Err(Error::type_error(
                "PlainMonthDay.toPlainDate requires an object",
            ));
        };
        let (era, era_year) = self.temporal_calendar_era_fields(value, month_day.calendar())?;
        let year = self.plain_date_optional_i64(value, "year")?;
        let era_year = era_year
            .map(|value| {
                value
                    .to_i32()
                    .ok_or_else(|| Self::plain_month_day_range("eraYear"))
            })
            .transpose()?;
        let year = year
            .map(|value| {
                value
                    .to_i32()
                    .ok_or_else(|| Self::plain_month_day_range("year"))
            })
            .transpose()?;
        let date = month_day
            .to_plain_date(Some(
                CalendarFields::new()
                    .with_era(era)
                    .with_era_year(era_year)
                    .with_optional_year(year),
            ))
            .map_err(temporal_error)?;
        self.create_plain_date_value(date)
    }

    fn plain_month_day_temporal(&self, value: &Value) -> Result<Option<PlainMonthDay>> {
        let Value::Object(id) = value else {
            return Ok(None);
        };
        match self.objects.temporal_value(*id)? {
            Some(TemporalValue::PlainMonthDay(month_day)) => Ok(Some(month_day.clone())),
            _ => Ok(None),
        }
    }

    fn plain_month_day_receiver(&self, value: &Value) -> Result<PlainMonthDay> {
        self.plain_month_day_temporal(value)?.ok_or_else(|| {
            Error::type_error("Temporal.PlainMonthDay method requires a PlainMonthDay receiver")
        })
    }

    fn create_plain_month_day_value(&mut self, value: PlainMonthDay) -> Result<Value> {
        self.create_temporal_calendar_value(
            TemporalValue::PlainMonthDay(value),
            TemporalFunctionKind::PlainMonthDayConstructor,
        )
    }

    fn plain_month_day_string(
        &mut self,
        receiver: &Value,
        display: DisplayCalendar,
    ) -> Result<Value> {
        let value = self.plain_month_day_receiver(receiver)?;
        self.heap_string_value(&value.to_ixdtf_string(display))
    }

    fn plain_month_day_fields(
        day: i64,
        era: Option<TinyAsciiStr<19>>,
        era_year: Option<i64>,
        month: Option<i64>,
        month_code: Option<temporal_rs::MonthCode>,
        year: Option<i64>,
        overflow: Overflow,
    ) -> Result<CalendarFields> {
        Self::plain_month_day_optional_fields(
            Some(day),
            era,
            era_year,
            month,
            month_code,
            year,
            overflow,
        )
    }

    fn plain_month_day_optional_fields(
        day: Option<i64>,
        era: Option<TinyAsciiStr<19>>,
        era_year: Option<i64>,
        month: Option<i64>,
        month_code: Option<temporal_rs::MonthCode>,
        year: Option<i64>,
        overflow: Overflow,
    ) -> Result<CalendarFields> {
        Ok(CalendarFields::new()
            .with_era(era)
            .with_era_year(
                era_year
                    .map(|value| {
                        value
                            .to_i32()
                            .ok_or_else(|| Self::plain_month_day_range("eraYear"))
                    })
                    .transpose()?,
            )
            .with_optional_year(
                year.map(|value| {
                    value
                        .to_i32()
                        .ok_or_else(|| Self::plain_month_day_range("year"))
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
            ))
    }

    fn plain_month_day_range(field: &str) -> Error {
        Error::exception(
            ErrorName::RangeError,
            format!("PlainMonthDay {field} is invalid"),
        )
    }
}
