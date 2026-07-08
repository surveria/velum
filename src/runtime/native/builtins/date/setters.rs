use crate::{
    error::Result,
    runtime::{Context, call::RuntimeCallArgs, object::DateValue},
    value::Value,
};

use super::support::{
    DateParts, date_value_to_number, integer_component, integer_component_with_default,
    make_date_value,
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

impl Context {
    pub(in crate::runtime::native) fn eval_date_prototype_set_full_year(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_date_prototype_set_component(args, this_value, DateSetterKind::FullYear)
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
        let date = Self::date_value_after_setter(current, args.as_slice(), kind)?;
        self.objects.set_date_value(id, date)?;
        date_value_to_number(date).map(Value::Number)
    }

    fn date_value_after_setter(
        current: DateValue,
        args: &[Value],
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

    fn date_value_after_set_full_year(current: DateValue, args: &[Value]) -> Result<DateValue> {
        let Some(year) = integer_component(args.first())? else {
            return Ok(DateValue::Invalid);
        };
        let parts = Self::date_parts_or_epoch(current)?;
        let Some(month) = integer_component_with_default(args.get(1), parts.month)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(date) = integer_component_with_default(args.get(2), parts.date)? else {
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

    fn date_value_after_set_month(current: DateValue, args: &[Value]) -> Result<DateValue> {
        let Some(month) = integer_component(args.first())? else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(date) = integer_component_with_default(args.get(1), parts.date)? else {
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

    fn date_value_after_set_date(current: DateValue, args: &[Value]) -> Result<DateValue> {
        let Some(date) = integer_component(args.first())? else {
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

    fn date_value_after_set_hours(current: DateValue, args: &[Value]) -> Result<DateValue> {
        let Some(hour) = integer_component(args.first())? else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(minute) = integer_component_with_default(args.get(1), parts.minute)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(second) = integer_component_with_default(args.get(2), parts.second)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(millisecond) = integer_component_with_default(args.get(3), parts.millisecond)?
        else {
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

    fn date_value_after_set_minutes(current: DateValue, args: &[Value]) -> Result<DateValue> {
        let Some(minute) = integer_component(args.first())? else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(second) = integer_component_with_default(args.get(1), parts.second)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(millisecond) = integer_component_with_default(args.get(2), parts.millisecond)?
        else {
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

    fn date_value_after_set_seconds(current: DateValue, args: &[Value]) -> Result<DateValue> {
        let Some(second) = integer_component(args.first())? else {
            return Ok(DateValue::Invalid);
        };
        let Some(parts) = Self::date_parts_or_valid(current)? else {
            return Ok(DateValue::Invalid);
        };
        let Some(millisecond) = integer_component_with_default(args.get(1), parts.millisecond)?
        else {
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

    fn date_value_after_set_milliseconds(current: DateValue, args: &[Value]) -> Result<DateValue> {
        let Some(millisecond) = integer_component(args.first())? else {
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
