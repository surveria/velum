use core::cmp::Ordering;

use temporal_rs::{PlainTime, options::ToStringRoundingOptions, partial::PartialTime};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::Value,
};

use super::temporal_error;

pub(super) const STATIC_METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("from", TemporalFunctionKind::PlainTimeFrom),
    ("compare", TemporalFunctionKind::PlainTimeCompare),
];

pub(super) const ACCESSORS: &[(&str, TemporalFunctionKind)] = &[
    ("hour", TemporalFunctionKind::PlainTimePrototypeHour),
    ("minute", TemporalFunctionKind::PlainTimePrototypeMinute),
    ("second", TemporalFunctionKind::PlainTimePrototypeSecond),
    (
        "millisecond",
        TemporalFunctionKind::PlainTimePrototypeMillisecond,
    ),
    (
        "microsecond",
        TemporalFunctionKind::PlainTimePrototypeMicrosecond,
    ),
    (
        "nanosecond",
        TemporalFunctionKind::PlainTimePrototypeNanosecond,
    ),
];

pub(super) const METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("with", TemporalFunctionKind::PlainTimePrototypeWith),
    ("add", TemporalFunctionKind::PlainTimePrototypeAdd),
    ("subtract", TemporalFunctionKind::PlainTimePrototypeSubtract),
    ("until", TemporalFunctionKind::PlainTimePrototypeUntil),
    ("since", TemporalFunctionKind::PlainTimePrototypeSince),
    ("round", TemporalFunctionKind::PlainTimePrototypeRound),
    ("equals", TemporalFunctionKind::PlainTimePrototypeEquals),
    ("toString", TemporalFunctionKind::PlainTimePrototypeToString),
    (
        "toLocaleString",
        TemporalFunctionKind::PlainTimePrototypeToLocaleString,
    ),
    ("toJSON", TemporalFunctionKind::PlainTimePrototypeToJson),
    ("valueOf", TemporalFunctionKind::PlainTimePrototypeValueOf),
];

impl Context {
    pub(super) fn eval_plain_time_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        if let Some(result) = self.eval_plain_time_accessor(kind, receiver) {
            return result;
        }
        match kind {
            TemporalFunctionKind::PlainTimeConstructor => Err(Error::type_error(
                "Temporal.PlainTime constructor requires 'new'",
            )),
            TemporalFunctionKind::PlainTimeFrom => self.eval_plain_time_from(args),
            TemporalFunctionKind::PlainTimeCompare => self.eval_plain_time_compare(args),
            TemporalFunctionKind::PlainTimePrototypeWith => {
                self.eval_plain_time_with(args, receiver)
            }
            TemporalFunctionKind::PlainTimePrototypeAdd => {
                self.eval_plain_time_add_subtract(args, receiver, false)
            }
            TemporalFunctionKind::PlainTimePrototypeSubtract => {
                self.eval_plain_time_add_subtract(args, receiver, true)
            }
            TemporalFunctionKind::PlainTimePrototypeUntil => {
                self.eval_plain_time_difference(args, receiver, false)
            }
            TemporalFunctionKind::PlainTimePrototypeSince => {
                self.eval_plain_time_difference(args, receiver, true)
            }
            TemporalFunctionKind::PlainTimePrototypeRound => {
                self.eval_plain_time_round(args, receiver)
            }
            TemporalFunctionKind::PlainTimePrototypeEquals => {
                self.eval_plain_time_equals(args, receiver)
            }
            TemporalFunctionKind::PlainTimePrototypeToString => {
                self.eval_plain_time_to_string(args, receiver)
            }
            TemporalFunctionKind::PlainTimePrototypeToLocaleString => {
                self.plain_time_receiver(receiver)?;
                self.format_temporal_locale_string(receiver, args)
            }
            TemporalFunctionKind::PlainTimePrototypeToJson => {
                self.plain_time_default_string(receiver)
            }
            TemporalFunctionKind::PlainTimePrototypeValueOf => Err(Error::type_error(
                "Temporal.PlainTime cannot be converted to a primitive",
            )),
            _ => Err(Error::runtime("PlainTime function kind was not handled")),
        }
    }

    fn eval_plain_time_accessor(
        &self,
        kind: TemporalFunctionKind,
        receiver: &Value,
    ) -> Option<Result<Value>> {
        let getter: fn(&PlainTime) -> f64 = match kind {
            TemporalFunctionKind::PlainTimePrototypeHour => |value| f64::from(value.hour()),
            TemporalFunctionKind::PlainTimePrototypeMinute => |value| f64::from(value.minute()),
            TemporalFunctionKind::PlainTimePrototypeSecond => |value| f64::from(value.second()),
            TemporalFunctionKind::PlainTimePrototypeMillisecond => {
                |value| f64::from(value.millisecond())
            }
            TemporalFunctionKind::PlainTimePrototypeMicrosecond => {
                |value| f64::from(value.microsecond())
            }
            TemporalFunctionKind::PlainTimePrototypeNanosecond => {
                |value| f64::from(value.nanosecond())
            }
            _ => return None,
        };
        Some(
            self.plain_time_receiver(receiver)
                .map(|value| Value::Number(getter(&value))),
        )
    }

    fn eval_plain_time_compare(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let one = self.plain_time_argument(values.first())?;
        let two = self.plain_time_argument(values.get(1))?;
        let result = match one.cmp(&two) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        };
        Ok(Value::Number(result))
    }

    fn eval_plain_time_from(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let value = values
            .first()
            .ok_or_else(|| Error::type_error("PlainTime.from requires an argument"))?;
        if self.plain_time_uses_property_bag(value)? {
            return self.create_plain_time_from_fields(value, values.get(1));
        }
        let time = self.plain_time_from_value(value)?;
        self.plain_date_overflow_option(values.get(1))?;
        self.create_plain_time_value(time)
    }

    fn plain_time_uses_property_bag(&self, value: &Value) -> Result<bool> {
        let Value::Object(id) = value else {
            return Ok(false);
        };
        Ok(self.objects.temporal_value(*id)?.is_none())
    }

    fn create_plain_time_from_fields(
        &mut self,
        value: &Value,
        options: Option<&Value>,
    ) -> Result<Value> {
        let hour = self.plain_date_optional_i64(value, "hour")?;
        let microsecond = self.plain_date_optional_i64(value, "microsecond")?;
        let millisecond = self.plain_date_optional_i64(value, "millisecond")?;
        let minute = self.plain_date_optional_i64(value, "minute")?;
        let nanosecond = self.plain_date_optional_i64(value, "nanosecond")?;
        let second = self.plain_date_optional_i64(value, "second")?;
        let overflow = self.plain_date_overflow_option(options)?;
        let partial = PartialTime::new()
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
        let time = PlainTime::from_partial(partial, Some(overflow)).map_err(temporal_error)?;
        self.create_plain_time_value(time)
    }

    fn eval_plain_time_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let time = self.plain_time_receiver(receiver)?;
        let values = args.as_slice();
        let Some(Value::Object(id)) = values.first() else {
            return Err(Error::type_error("PlainTime.with requires an object"));
        };
        if self.objects.temporal_value(*id)?.is_some() {
            return Err(Error::type_error(
                "PlainTime.with does not accept a Temporal object",
            ));
        }
        let object = Value::Object(*id);
        for name in ["calendar", "timeZone"] {
            if !matches!(self.get_named(&object, name)?, Value::Undefined) {
                return Err(Error::type_error(format!(
                    "PlainTime.with does not accept {name}"
                )));
            }
        }
        let hour = self.plain_date_optional_i64(&object, "hour")?;
        let microsecond = self.plain_date_optional_i64(&object, "microsecond")?;
        let millisecond = self.plain_date_optional_i64(&object, "millisecond")?;
        let minute = self.plain_date_optional_i64(&object, "minute")?;
        let nanosecond = self.plain_date_optional_i64(&object, "nanosecond")?;
        let second = self.plain_date_optional_i64(&object, "second")?;
        let overflow = self.plain_date_overflow_option(values.get(1))?;
        let partial = PartialTime::new()
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
        let result = time.with(partial, Some(overflow)).map_err(temporal_error)?;
        self.create_plain_time_value(result)
    }

    fn eval_plain_time_add_subtract(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        subtract: bool,
    ) -> Result<Value> {
        let time = self.plain_time_receiver(receiver)?;
        let duration = self.duration_from_value(args.as_slice().first())?;
        let result = if subtract {
            time.subtract(&duration)
        } else {
            time.add(&duration)
        }
        .map_err(temporal_error)?;
        self.create_plain_time_value(result)
    }

    fn eval_plain_time_difference(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        since: bool,
    ) -> Result<Value> {
        let values = args.as_slice();
        let time = self.plain_time_receiver(receiver)?;
        let other = self.plain_time_argument(values.first())?;
        let settings = self.plain_date_difference_settings(values.get(1))?;
        let result = if since {
            time.since(&other, settings)
        } else {
            time.until(&other, settings)
        }
        .map_err(temporal_error)?;
        self.create_duration_value(result)
    }

    fn eval_plain_time_round(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let time = self.plain_time_receiver(receiver)?;
        let options = self.plain_date_time_rounding_options(args.as_slice().first())?;
        let result = time.round(options).map_err(temporal_error)?;
        self.create_plain_time_value(result)
    }

    fn eval_plain_time_equals(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let time = self.plain_time_receiver(receiver)?;
        let other = self.plain_time_argument(args.as_slice().first())?;
        Ok(Value::Bool(time == other))
    }

    fn eval_plain_time_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let options = self.duration_to_string_options(args.as_slice().first())?;
        let time = self.plain_time_receiver(receiver)?;
        let text = time.to_ixdtf_string(options).map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    fn plain_time_default_string(&mut self, receiver: &Value) -> Result<Value> {
        let time = self.plain_time_receiver(receiver)?;
        let text = time
            .to_ixdtf_string(ToStringRoundingOptions::default())
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    fn plain_time_argument(&mut self, value: Option<&Value>) -> Result<PlainTime> {
        let value = value.ok_or_else(|| Error::type_error("PlainTime requires an argument"))?;
        self.plain_time_from_value(value)
    }

    fn plain_time_receiver(&self, value: &Value) -> Result<PlainTime> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(
                "Temporal.PlainTime method requires a PlainTime receiver",
            ));
        };
        match self.objects.temporal_value(*id)? {
            Some(TemporalValue::PlainTime(time)) => Ok(*time),
            _ => Err(Error::type_error(
                "Temporal.PlainTime method requires a PlainTime receiver",
            )),
        }
    }

    fn create_plain_time_value(&mut self, time: PlainTime) -> Result<Value> {
        self.create_temporal_calendar_value(
            TemporalValue::PlainTime(time),
            TemporalFunctionKind::PlainTimeConstructor,
        )
    }
}
