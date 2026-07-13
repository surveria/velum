use std::cmp::Ordering;

use num_traits::ToPrimitive;
use temporal_rs::{
    PlainYearMonth,
    fields::{CalendarFields, YearMonthCalendarFields},
    options::{DisplayCalendar, Overflow},
    partial::PartialYearMonth,
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::{ErrorName, Value},
};

use super::temporal_error;

pub(super) const STATIC_METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("from", TemporalFunctionKind::PlainYearMonthFrom),
    ("compare", TemporalFunctionKind::PlainYearMonthCompare),
];

pub(super) const ACCESSORS: &[(&str, TemporalFunctionKind)] = &[
    ("year", TemporalFunctionKind::PlainYearMonthPrototypeYear),
    ("month", TemporalFunctionKind::PlainYearMonthPrototypeMonth),
    (
        "monthCode",
        TemporalFunctionKind::PlainYearMonthPrototypeMonthCode,
    ),
    (
        "calendarId",
        TemporalFunctionKind::PlainYearMonthPrototypeCalendarId,
    ),
    ("era", TemporalFunctionKind::PlainYearMonthPrototypeEra),
    (
        "eraYear",
        TemporalFunctionKind::PlainYearMonthPrototypeEraYear,
    ),
    (
        "daysInMonth",
        TemporalFunctionKind::PlainYearMonthPrototypeDaysInMonth,
    ),
    (
        "daysInYear",
        TemporalFunctionKind::PlainYearMonthPrototypeDaysInYear,
    ),
    (
        "monthsInYear",
        TemporalFunctionKind::PlainYearMonthPrototypeMonthsInYear,
    ),
    (
        "inLeapYear",
        TemporalFunctionKind::PlainYearMonthPrototypeInLeapYear,
    ),
];

pub(super) const METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("with", TemporalFunctionKind::PlainYearMonthPrototypeWith),
    ("add", TemporalFunctionKind::PlainYearMonthPrototypeAdd),
    (
        "subtract",
        TemporalFunctionKind::PlainYearMonthPrototypeSubtract,
    ),
    ("until", TemporalFunctionKind::PlainYearMonthPrototypeUntil),
    ("since", TemporalFunctionKind::PlainYearMonthPrototypeSince),
    (
        "equals",
        TemporalFunctionKind::PlainYearMonthPrototypeEquals,
    ),
    (
        "toPlainDate",
        TemporalFunctionKind::PlainYearMonthPrototypeToPlainDate,
    ),
    (
        "toString",
        TemporalFunctionKind::PlainYearMonthPrototypeToString,
    ),
    (
        "toLocaleString",
        TemporalFunctionKind::PlainYearMonthPrototypeToLocaleString,
    ),
    (
        "toJSON",
        TemporalFunctionKind::PlainYearMonthPrototypeToJson,
    ),
    (
        "valueOf",
        TemporalFunctionKind::PlainYearMonthPrototypeValueOf,
    ),
];

impl Context {
    pub(super) fn eval_plain_year_month_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if let Some(result) = self.eval_plain_year_month_accessor(kind, receiver) {
            return result;
        }
        match kind {
            TemporalFunctionKind::PlainYearMonthConstructor => Err(Error::type_error(
                "Temporal.PlainYearMonth constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainYearMonthFrom => self.eval_plain_year_month_from(args),
            TemporalFunctionKind::PlainYearMonthCompare => self.eval_plain_year_month_compare(args),
            TemporalFunctionKind::PlainYearMonthPrototypeWith => {
                self.eval_plain_year_month_with(args, receiver)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeAdd => {
                self.eval_plain_year_month_add_subtract(args, receiver, false)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeSubtract => {
                self.eval_plain_year_month_add_subtract(args, receiver, true)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeUntil => {
                self.eval_plain_year_month_difference(args, receiver, false)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeSince => {
                self.eval_plain_year_month_difference(args, receiver, true)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeEquals => {
                self.eval_plain_year_month_equals(args, receiver)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeToPlainDate => {
                self.eval_plain_year_month_to_plain_date(args, receiver)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeToString => {
                let display = self.plain_date_display_calendar(args.as_slice().first())?;
                self.plain_year_month_string(receiver, display)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeToLocaleString => {
                self.format_temporal_locale_string(receiver, args)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeToJson => {
                self.plain_year_month_string(receiver, DisplayCalendar::Auto)
            }
            TemporalFunctionKind::PlainYearMonthPrototypeValueOf => Err(Error::type_error(
                "Temporal.PlainYearMonth cannot be converted to a primitive",
            )),
            _ => Err(Error::runtime(
                "PlainYearMonth function kind was not handled",
            )),
        }
    }

    fn eval_plain_year_month_accessor(
        &mut self,
        kind: TemporalFunctionKind,
        receiver: &Value,
    ) -> Option<Result<Value>> {
        if !matches!(
            kind,
            TemporalFunctionKind::PlainYearMonthPrototypeYear
                | TemporalFunctionKind::PlainYearMonthPrototypeMonth
                | TemporalFunctionKind::PlainYearMonthPrototypeMonthCode
                | TemporalFunctionKind::PlainYearMonthPrototypeCalendarId
                | TemporalFunctionKind::PlainYearMonthPrototypeEra
                | TemporalFunctionKind::PlainYearMonthPrototypeEraYear
                | TemporalFunctionKind::PlainYearMonthPrototypeDaysInMonth
                | TemporalFunctionKind::PlainYearMonthPrototypeDaysInYear
                | TemporalFunctionKind::PlainYearMonthPrototypeMonthsInYear
                | TemporalFunctionKind::PlainYearMonthPrototypeInLeapYear
        ) {
            return None;
        }
        let value = match self.plain_year_month_receiver(receiver) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        };
        let result = match kind {
            TemporalFunctionKind::PlainYearMonthPrototypeYear => {
                Value::Number(f64::from(value.year()))
            }
            TemporalFunctionKind::PlainYearMonthPrototypeMonth => {
                Value::Number(f64::from(value.month()))
            }
            TemporalFunctionKind::PlainYearMonthPrototypeMonthCode => {
                return Some(self.heap_string_value(value.month_code().as_str()));
            }
            TemporalFunctionKind::PlainYearMonthPrototypeCalendarId => {
                return Some(self.heap_string_value(value.calendar_id()));
            }
            TemporalFunctionKind::PlainYearMonthPrototypeEra => {
                return Some(value.era().map_or(Ok(Value::Undefined), |era| {
                    self.heap_string_value(era.as_str())
                }));
            }
            TemporalFunctionKind::PlainYearMonthPrototypeEraYear => {
                return Some(Ok(value
                    .era_year()
                    .map_or(Value::Undefined, |year| Value::Number(f64::from(year)))));
            }
            TemporalFunctionKind::PlainYearMonthPrototypeDaysInMonth => {
                Value::Number(f64::from(value.days_in_month()))
            }
            TemporalFunctionKind::PlainYearMonthPrototypeDaysInYear => {
                Value::Number(f64::from(value.days_in_year()))
            }
            TemporalFunctionKind::PlainYearMonthPrototypeMonthsInYear => {
                Value::Number(f64::from(value.months_in_year()))
            }
            TemporalFunctionKind::PlainYearMonthPrototypeInLeapYear => {
                Value::Bool(value.in_leap_year())
            }
            _ => return None,
        };
        Some(Ok(result))
    }

    fn eval_plain_year_month_from(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let value = values
            .first()
            .ok_or_else(|| Error::type_error("PlainYearMonth.from requires an argument"))?;
        let year_month = if let Some(resolved) = self.plain_year_month_temporal(value)? {
            self.plain_date_overflow_option(values.get(1))?;
            resolved
        } else if let Some(text) = value.string_text() {
            let parsed = PlainYearMonth::from_utf8(text.as_bytes()).map_err(temporal_error)?;
            self.plain_date_overflow_option(values.get(1))?;
            parsed
        } else {
            self.plain_year_month_from_fields(value, values.get(1))?
        };
        self.create_plain_year_month_value(year_month)
    }

    fn plain_year_month_from_fields(
        &mut self,
        value: &Value,
        options: Option<&Value>,
    ) -> Result<PlainYearMonth> {
        let Value::Object(_) = value else {
            return Err(Error::type_error(
                "PlainYearMonth input must be a string or object",
            ));
        };
        let calendar_value = self.get_named(value, "calendar")?;
        let calendar = self.temporal_calendar(Some(&calendar_value))?;
        if !matches!(calendar.identifier(), "iso8601" | "chinese" | "dangi") {
            let era = self.get_named(value, "era")?;
            let era_year = self.get_named(value, "eraYear")?;
            if matches!(era, Value::Undefined) != matches!(era_year, Value::Undefined) {
                return Err(Error::type_error(
                    "PlainYearMonth era and eraYear must be provided together",
                ));
            }
            if !matches!(era, Value::Undefined) {
                self.to_string(&era)?;
                let number = self.to_number(&era_year)?;
                if !number.is_finite() {
                    return Err(Self::plain_year_month_range("eraYear"));
                }
            }
        }
        let month = self.plain_date_optional_i64(value, "month")?;
        let month_code_value = self.get_named(value, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_date_month_code(&month_code_value)?)
        };
        let year = self.plain_date_required_i64(value, "year")?;
        let overflow = self.plain_date_overflow_option(options)?;
        let fields = Self::plain_year_month_fields(Some(year), month, month_code, overflow)?;
        PlainYearMonth::from_partial(
            PartialYearMonth {
                calendar_fields: fields,
                calendar,
            },
            Some(overflow),
        )
        .map_err(temporal_error)
    }

    fn eval_plain_year_month_compare(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let one = self.plain_year_month_argument(values.first())?;
        let two = self.plain_year_month_argument(values.get(1))?;
        let result = match one.compare_iso(&two) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        };
        Ok(Value::Number(result))
    }

    fn eval_plain_year_month_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let year_month = self.plain_year_month_receiver(receiver)?;
        let values = args.as_slice();
        let Some(Value::Object(id)) = values.first() else {
            return Err(Error::type_error("PlainYearMonth.with requires an object"));
        };
        if self.objects.temporal_value(*id)?.is_some() {
            return Err(Error::type_error(
                "PlainYearMonth.with does not accept a Temporal object",
            ));
        }
        let object = Value::Object(*id);
        for name in ["calendar", "timeZone"] {
            if !matches!(self.get_named(&object, name)?, Value::Undefined) {
                return Err(Error::type_error(format!(
                    "PlainYearMonth.with does not accept {name}"
                )));
            }
        }
        let month = self.plain_date_optional_i64(&object, "month")?;
        let month_code_value = self.get_named(&object, "monthCode")?;
        let month_code = if matches!(month_code_value, Value::Undefined) {
            None
        } else {
            Some(self.plain_date_month_code(&month_code_value)?)
        };
        let year = self.plain_date_optional_i64(&object, "year")?;
        if month.is_some_and(|value| value <= 0) {
            return Err(Self::plain_year_month_range("month"));
        }
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let fields = Self::plain_year_month_fields(year, month, month_code, overflow)?;
        let result = year_month
            .with(fields, Some(overflow))
            .map_err(temporal_error)?;
        self.create_plain_year_month_value(result)
    }

    fn eval_plain_year_month_add_subtract(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        subtract: bool,
    ) -> Result<Value> {
        let year_month = self.plain_year_month_receiver(receiver)?;
        let values = args.as_slice();
        let duration = self.duration_from_value(values.first())?;
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let result = if subtract {
            year_month.subtract(&duration, overflow)
        } else {
            year_month.add(&duration, overflow)
        }
        .map_err(temporal_error)?;
        self.create_plain_year_month_value(result)
    }

    fn eval_plain_year_month_difference(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        since: bool,
    ) -> Result<Value> {
        let values = args.as_slice();
        let year_month = self.plain_year_month_receiver(receiver)?;
        let other = self.plain_year_month_argument(values.first())?;
        let settings = self.plain_date_difference_settings(values.get(1))?;
        let duration = if since {
            year_month.since(&other, settings)
        } else {
            year_month.until(&other, settings)
        }
        .map_err(temporal_error)?;
        self.create_duration_value(duration)
    }

    fn eval_plain_year_month_equals(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let one = self.plain_year_month_receiver(receiver)?;
        let two = self.plain_year_month_argument(args.as_slice().first())?;
        Ok(Value::Bool(one == two))
    }

    fn eval_plain_year_month_to_plain_date(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let year_month = self.plain_year_month_receiver(receiver)?;
        let Some(value @ Value::Object(_)) = args.as_slice().first() else {
            return Err(Error::type_error(
                "PlainYearMonth.toPlainDate requires an object",
            ));
        };
        let day = self.plain_date_required_i64(value, "day")?;
        let day = Self::plain_date_u8_field(day, "day", Overflow::Constrain)?;
        let date = year_month
            .to_plain_date(Some(CalendarFields::new().with_day(day)))
            .map_err(temporal_error)?;
        self.create_plain_date_value(date)
    }

    fn plain_year_month_argument(&mut self, value: Option<&Value>) -> Result<PlainYearMonth> {
        let value =
            value.ok_or_else(|| Error::type_error("PlainYearMonth requires an argument"))?;
        if let Some(resolved) = self.plain_year_month_temporal(value)? {
            return Ok(resolved);
        }
        if let Some(text) = value.string_text() {
            return PlainYearMonth::from_utf8(text.as_bytes()).map_err(temporal_error);
        }
        self.plain_year_month_from_fields(value, None)
    }

    fn plain_year_month_temporal(&self, value: &Value) -> Result<Option<PlainYearMonth>> {
        let Value::Object(id) = value else {
            return Ok(None);
        };
        match self.objects.temporal_value(*id)? {
            Some(TemporalValue::PlainYearMonth(year_month)) => Ok(Some(year_month.clone())),
            _ => Ok(None),
        }
    }

    fn plain_year_month_receiver(&self, value: &Value) -> Result<PlainYearMonth> {
        self.plain_year_month_temporal(value)?.ok_or_else(|| {
            Error::type_error("Temporal.PlainYearMonth method requires a PlainYearMonth receiver")
        })
    }

    fn create_plain_year_month_value(&mut self, value: PlainYearMonth) -> Result<Value> {
        self.create_temporal_calendar_value(
            TemporalValue::PlainYearMonth(value),
            TemporalFunctionKind::PlainYearMonthConstructor,
        )
    }

    fn plain_year_month_string(
        &mut self,
        receiver: &Value,
        display: DisplayCalendar,
    ) -> Result<Value> {
        let value = self.plain_year_month_receiver(receiver)?;
        self.heap_string_value(&value.to_ixdtf_string(display))
    }

    fn plain_year_month_fields(
        year: Option<i64>,
        month: Option<i64>,
        month_code: Option<temporal_rs::MonthCode>,
        overflow: Overflow,
    ) -> Result<YearMonthCalendarFields> {
        Ok(YearMonthCalendarFields::new()
            .with_optional_year(
                year.map(|value| {
                    value
                        .to_i32()
                        .ok_or_else(|| Self::plain_year_month_range("year"))
                })
                .transpose()?,
            )
            .with_optional_month(
                month
                    .map(|value| Self::plain_date_u8_field(value, "month", overflow))
                    .transpose()?,
            )
            .with_optional_month_code(month_code))
    }

    fn plain_year_month_range(field: &str) -> Error {
        Error::exception(
            ErrorName::RangeError,
            format!("PlainYearMonth {field} is invalid"),
        )
    }
}
