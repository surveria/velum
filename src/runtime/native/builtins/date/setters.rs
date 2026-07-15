use crate::{
    error::Result,
    runtime::{Context, call::RuntimeCallArgs, object::DateValue},
    value::Value,
};

use super::support::{
    DateParts, date_value_to_number, integer_component, make_date_value, normalize_component_year,
};

#[derive(Debug, Clone, Copy)]
enum DateSetterKind {
    FullYear,
    Month,
    Date,
    Hours,
    Minutes,
    Seconds,
    Milliseconds,
}

impl DateSetterKind {
    const fn maximum_argument_count(self) -> usize {
        match self {
            Self::FullYear | Self::Minutes => 3,
            Self::Month | Self::Seconds => 2,
            Self::Date | Self::Milliseconds => 1,
            Self::Hours => 4,
        }
    }

    const fn uses_epoch_for_invalid_date(self) -> bool {
        matches!(self, Self::FullYear)
    }
}

impl Context {
    pub(in crate::runtime::native) fn eval_date_prototype_set_full_year(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_set_component(args, this_value, DateSetterKind::FullYear)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_set_year(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let (id, current) = self.date_this_object_value(this_value)?;
        let Some(year) = integer_component(self, args.as_slice().first())? else {
            self.objects.set_date_value(id, DateValue::Invalid)?;
            return Ok(Value::Number(f64::NAN));
        };
        let parts = Self::date_parts_or_epoch(current)?;
        let date = make_date_value(
            normalize_component_year(year),
            parts.month,
            parts.date,
            parts.hour,
            parts.minute,
            parts.second,
            parts.millisecond,
        );
        self.objects.set_date_value(id, date)?;
        date_value_to_number(date).map(Value::Number)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_set_month(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_set_component(args, this_value, DateSetterKind::Month)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_set_date(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_set_component(args, this_value, DateSetterKind::Date)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_set_hours(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_set_component(args, this_value, DateSetterKind::Hours)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_set_minutes(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_set_component(args, this_value, DateSetterKind::Minutes)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_set_seconds(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_set_component(args, this_value, DateSetterKind::Seconds)
    }

    pub(in crate::runtime::native) fn eval_date_prototype_set_milliseconds(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_set_component(args, this_value, DateSetterKind::Milliseconds)
    }

    fn eval_date_prototype_set_component(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        kind: DateSetterKind,
    ) -> Result<Value> {
        let (id, current) = self.date_this_object_value(this_value)?;
        let components = self.coerce_date_setter_arguments(args.as_slice(), kind)?;
        if current.millis().is_none() && !kind.uses_epoch_for_invalid_date() {
            return Ok(Value::Number(f64::NAN));
        }
        let date = Self::date_value_after_setter(current, &components, kind)?;
        self.objects.set_date_value(id, date)?;
        date_value_to_number(date).map(Value::Number)
    }

    fn coerce_date_setter_arguments(
        &mut self,
        args: &[Value],
        kind: DateSetterKind,
    ) -> Result<Vec<Option<i64>>> {
        let mut components = Vec::with_capacity(kind.maximum_argument_count());
        for argument in args.iter().take(kind.maximum_argument_count()) {
            components.push(integer_component(self, Some(argument))?);
        }
        Ok(components)
    }

    fn date_value_after_setter(
        current: DateValue,
        args: &[Option<i64>],
        kind: DateSetterKind,
    ) -> Result<DateValue> {
        match kind {
            DateSetterKind::FullYear => Self::date_value_after_set_full_year(current, args),
            DateSetterKind::Month => Self::date_value_after_set_month(current, args),
            DateSetterKind::Date => Self::date_value_after_set_date(current, args),
            DateSetterKind::Hours => Self::date_value_after_set_hours(current, args),
            DateSetterKind::Minutes => Self::date_value_after_set_minutes(current, args),
            DateSetterKind::Seconds => Self::date_value_after_set_seconds(current, args),
            DateSetterKind::Milliseconds => Self::date_value_after_set_milliseconds(current, args),
        }
    }

    fn date_value_after_set_full_year(
        current: DateValue,
        args: &[Option<i64>],
    ) -> Result<DateValue> {
        let Some(year) = Self::date_setter_component(args, 0) else {
            return Ok(DateValue::Invalid);
        };
        let parts = Self::date_parts_or_epoch(current)?;
        let Some(month) = Self::date_setter_component_or(args, 1, parts.month) else {
            return Ok(DateValue::Invalid);
        };
        let Some(date) = Self::date_setter_component_or(args, 2, parts.date) else {
            return Ok(DateValue::Invalid);
        };
        Ok(make_date_value(
            year,
            month,
            date,
            parts.hour,
            parts.minute,
            parts.second,
            parts.millisecond,
        ))
    }

    fn date_value_after_set_month(current: DateValue, args: &[Option<i64>]) -> Result<DateValue> {
        let Some(month) = Self::date_setter_component(args, 0) else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(date) = Self::date_setter_component_or(args, 1, parts.date) else {
            return Ok(DateValue::Invalid);
        };
        Ok(make_date_value(
            parts.year,
            month,
            date,
            parts.hour,
            parts.minute,
            parts.second,
            parts.millisecond,
        ))
    }

    fn date_value_after_set_date(current: DateValue, args: &[Option<i64>]) -> Result<DateValue> {
        let Some(date) = Self::date_setter_component(args, 0) else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        Ok(make_date_value(
            parts.year,
            parts.month,
            date,
            parts.hour,
            parts.minute,
            parts.second,
            parts.millisecond,
        ))
    }

    fn date_value_after_set_hours(current: DateValue, args: &[Option<i64>]) -> Result<DateValue> {
        let Some(hour) = Self::date_setter_component(args, 0) else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(minute) = Self::date_setter_component_or(args, 1, parts.minute) else {
            return Ok(DateValue::Invalid);
        };
        let Some(second) = Self::date_setter_component_or(args, 2, parts.second) else {
            return Ok(DateValue::Invalid);
        };
        let Some(millisecond) = Self::date_setter_component_or(args, 3, parts.millisecond) else {
            return Ok(DateValue::Invalid);
        };
        Ok(make_date_value(
            parts.year,
            parts.month,
            parts.date,
            hour,
            minute,
            second,
            millisecond,
        ))
    }

    fn date_value_after_set_minutes(current: DateValue, args: &[Option<i64>]) -> Result<DateValue> {
        let Some(minute) = Self::date_setter_component(args, 0) else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(second) = Self::date_setter_component_or(args, 1, parts.second) else {
            return Ok(DateValue::Invalid);
        };
        let Some(millisecond) = Self::date_setter_component_or(args, 2, parts.millisecond) else {
            return Ok(DateValue::Invalid);
        };
        Ok(make_date_value(
            parts.year,
            parts.month,
            parts.date,
            parts.hour,
            minute,
            second,
            millisecond,
        ))
    }

    fn date_value_after_set_seconds(current: DateValue, args: &[Option<i64>]) -> Result<DateValue> {
        let Some(second) = Self::date_setter_component(args, 0) else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(millisecond) = Self::date_setter_component_or(args, 1, parts.millisecond) else {
            return Ok(DateValue::Invalid);
        };
        Ok(make_date_value(
            parts.year,
            parts.month,
            parts.date,
            parts.hour,
            parts.minute,
            second,
            millisecond,
        ))
    }

    fn date_value_after_set_milliseconds(
        current: DateValue,
        args: &[Option<i64>],
    ) -> Result<DateValue> {
        let Some(millisecond) = Self::date_setter_component(args, 0) else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        Ok(make_date_value(
            parts.year,
            parts.month,
            parts.date,
            parts.hour,
            parts.minute,
            parts.second,
            millisecond,
        ))
    }

    fn date_setter_component(args: &[Option<i64>], index: usize) -> Option<i64> {
        args.get(index).copied().flatten()
    }

    fn date_setter_component_or(args: &[Option<i64>], index: usize, default: i64) -> Option<i64> {
        args.get(index).copied().unwrap_or(Some(default))
    }

    fn date_parts_or_epoch(value: DateValue) -> Result<DateParts> {
        let Some(ms) = value.millis() else {
            return DateParts::from_millis(0);
        };
        DateParts::from_millis(ms)
    }

    fn date_parts_or_valid(value: DateValue) -> Result<Option<DateParts>> {
        let Some(ms) = value.millis() else {
            return Ok(None);
        };
        DateParts::from_millis(ms).map(Some)
    }
}
