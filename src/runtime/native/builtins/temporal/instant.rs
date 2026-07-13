use std::cmp::Ordering;

use num_traits::ToPrimitive;
use temporal_rs::{Instant, TimeZone, options::ToStringRoundingOptions};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::{ErrorName, JsBigInt, Value},
};

use super::temporal_error;

pub(super) const STATIC_METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("from", TemporalFunctionKind::InstantFrom),
    (
        "fromEpochMilliseconds",
        TemporalFunctionKind::InstantFromEpochMilliseconds,
    ),
    (
        "fromEpochNanoseconds",
        TemporalFunctionKind::InstantFromEpochNanoseconds,
    ),
    ("compare", TemporalFunctionKind::InstantCompare),
];

pub(super) const ACCESSORS: &[(&str, TemporalFunctionKind)] = &[
    (
        "epochMilliseconds",
        TemporalFunctionKind::InstantPrototypeEpochMilliseconds,
    ),
    (
        "epochNanoseconds",
        TemporalFunctionKind::InstantPrototypeEpochNanoseconds,
    ),
];

pub(super) const METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("add", TemporalFunctionKind::InstantPrototypeAdd),
    ("subtract", TemporalFunctionKind::InstantPrototypeSubtract),
    ("until", TemporalFunctionKind::InstantPrototypeUntil),
    ("since", TemporalFunctionKind::InstantPrototypeSince),
    ("round", TemporalFunctionKind::InstantPrototypeRound),
    ("equals", TemporalFunctionKind::InstantPrototypeEquals),
    (
        "toZonedDateTimeISO",
        TemporalFunctionKind::InstantPrototypeToZonedDateTimeIso,
    ),
    ("toString", TemporalFunctionKind::InstantPrototypeToString),
    (
        "toLocaleString",
        TemporalFunctionKind::InstantPrototypeToLocaleString,
    ),
    ("toJSON", TemporalFunctionKind::InstantPrototypeToJson),
    ("valueOf", TemporalFunctionKind::InstantPrototypeValueOf),
];

impl Context {
    pub(super) fn eval_instant_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        match kind {
            TemporalFunctionKind::InstantConstructor => Err(Error::type_error(
                "Temporal.Instant constructor requires 'new'",
            )),
            TemporalFunctionKind::InstantFrom => self.eval_instant_from(args),
            TemporalFunctionKind::InstantFromEpochMilliseconds => {
                self.eval_instant_from_epoch_milliseconds(args)
            }
            TemporalFunctionKind::InstantFromEpochNanoseconds => {
                self.eval_instant_from_epoch_nanoseconds(args)
            }
            TemporalFunctionKind::InstantCompare => self.eval_instant_compare(args),
            TemporalFunctionKind::InstantPrototypeEpochMilliseconds => {
                let instant = self.instant_receiver(receiver)?;
                instant
                    .epoch_milliseconds()
                    .to_f64()
                    .map(Value::Number)
                    .ok_or_else(|| Error::runtime("Instant milliseconds cannot become Number"))
            }
            TemporalFunctionKind::InstantPrototypeEpochNanoseconds => {
                let instant = self.instant_receiver(receiver)?;
                Self::instant_bigint_value(instant.as_i128())
            }
            TemporalFunctionKind::InstantPrototypeAdd => {
                self.eval_instant_add_subtract(args, receiver, false)
            }
            TemporalFunctionKind::InstantPrototypeSubtract => {
                self.eval_instant_add_subtract(args, receiver, true)
            }
            TemporalFunctionKind::InstantPrototypeUntil => {
                self.eval_instant_difference(args, receiver, false)
            }
            TemporalFunctionKind::InstantPrototypeSince => {
                self.eval_instant_difference(args, receiver, true)
            }
            TemporalFunctionKind::InstantPrototypeRound => self.eval_instant_round(args, receiver),
            TemporalFunctionKind::InstantPrototypeEquals => {
                self.eval_instant_equals(args, receiver)
            }
            TemporalFunctionKind::InstantPrototypeToZonedDateTimeIso => {
                self.eval_instant_to_zoned_date_time(args, receiver)
            }
            TemporalFunctionKind::InstantPrototypeToString => {
                self.eval_instant_to_string(args, receiver)
            }
            TemporalFunctionKind::InstantPrototypeToLocaleString => {
                self.instant_receiver(receiver)?;
                self.format_temporal_locale_string(receiver, args)
            }
            TemporalFunctionKind::InstantPrototypeToJson => self.instant_default_string(receiver),
            TemporalFunctionKind::InstantPrototypeValueOf => Err(Error::type_error(
                "Temporal.Instant cannot be converted to a primitive",
            )),
            _ => Err(Error::runtime("Instant function kind was not handled")),
        }
    }

    fn eval_instant_from(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let instant = self.instant_argument(args.as_slice().first())?;
        self.create_instant_value(instant)
    }

    fn eval_instant_from_epoch_milliseconds(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let number = self.to_number(&value)?;
        if !number.is_finite() || number.fract() != 0.0 {
            return Err(Self::instant_range("epochMilliseconds"));
        }
        let milliseconds = number
            .to_i64()
            .ok_or_else(|| Self::instant_range("epochMilliseconds"))?;
        let instant = Instant::from_epoch_milliseconds(milliseconds).map_err(temporal_error)?;
        self.create_instant_value(instant)
    }

    pub(in crate::runtime::native) fn create_instant_from_epoch_milliseconds_value(
        &mut self,
        milliseconds: i64,
    ) -> Result<Value> {
        let instant = Instant::from_epoch_milliseconds(milliseconds).map_err(temporal_error)?;
        self.create_instant_value(instant)
    }

    fn eval_instant_from_epoch_nanoseconds(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let nanos = self.instant_bigint_argument(args.as_slice().first())?;
        let instant = Instant::try_new(nanos).map_err(temporal_error)?;
        self.create_instant_value(instant)
    }

    fn eval_instant_compare(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = args.as_slice();
        let one = self.instant_argument(values.first())?;
        let two = self.instant_argument(values.get(1))?;
        let result = match one.as_i128().cmp(&two.as_i128()) {
            Ordering::Less => -1.0,
            Ordering::Equal => 0.0,
            Ordering::Greater => 1.0,
        };
        Ok(Value::Number(result))
    }

    fn eval_instant_add_subtract(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        subtract: bool,
    ) -> Result<Value> {
        let instant = self.instant_receiver(receiver)?;
        let duration = self.duration_from_value(args.as_slice().first())?;
        let result = if subtract {
            instant.subtract(&duration)
        } else {
            instant.add(&duration)
        }
        .map_err(temporal_error)?;
        self.create_instant_value(result)
    }

    fn eval_instant_difference(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
        since: bool,
    ) -> Result<Value> {
        let values = args.as_slice();
        let instant = self.instant_receiver(receiver)?;
        let other = self.instant_argument(values.first())?;
        let settings = self.plain_date_difference_settings(values.get(1))?;
        let duration = if since {
            instant.since(&other, settings)
        } else {
            instant.until(&other, settings)
        }
        .map_err(temporal_error)?;
        self.create_duration_value(duration)
    }

    fn eval_instant_round(&mut self, args: RuntimeCallArgs<'_>, receiver: &Value) -> Result<Value> {
        let instant = self.instant_receiver(receiver)?;
        let options = self.plain_date_time_rounding_options(args.as_slice().first())?;
        let rounded = instant.round(options).map_err(temporal_error)?;
        self.create_instant_value(rounded)
    }

    fn eval_instant_equals(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let instant = self.instant_receiver(receiver)?;
        let other = self.instant_argument(args.as_slice().first())?;
        Ok(Value::Bool(instant == other))
    }

    fn eval_instant_to_zoned_date_time(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let instant = self.instant_receiver(receiver)?;
        let value = args
            .as_slice()
            .first()
            .ok_or_else(|| Error::type_error("Instant.toZonedDateTimeISO requires a time zone"))?;
        let text = value
            .string_text()
            .ok_or_else(|| Error::type_error("Temporal time zone must be a string"))?;
        let zone = TimeZone::try_from_str(text).map_err(temporal_error)?;
        let zoned = instant
            .to_zoned_date_time_iso(zone)
            .map_err(temporal_error)?;
        self.create_temporal_calendar_value(
            TemporalValue::ZonedDateTime(zoned),
            TemporalFunctionKind::ZonedDateTimeConstructor,
        )
    }

    fn eval_instant_to_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let value = args.as_slice().first();
        let options = self.duration_to_string_options(value)?;
        let time_zone = self.instant_time_zone_option(value)?;
        let instant = self.instant_receiver(receiver)?;
        let text = instant
            .to_ixdtf_string(time_zone, options)
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    fn instant_time_zone_option(&mut self, value: Option<&Value>) -> Result<Option<TimeZone>> {
        let Some(value @ Value::Object(_)) = value else {
            return Ok(None);
        };
        let zone = self.get_named(value, "timeZone")?;
        if matches!(zone, Value::Undefined) {
            return Ok(None);
        }
        let text = zone
            .string_text()
            .ok_or_else(|| Error::type_error("Temporal time zone must be a string"))?;
        TimeZone::try_from_str(text)
            .map(Some)
            .map_err(temporal_error)
    }

    fn instant_default_string(&mut self, receiver: &Value) -> Result<Value> {
        let instant = self.instant_receiver(receiver)?;
        let text = instant
            .to_ixdtf_string(None, ToStringRoundingOptions::default())
            .map_err(temporal_error)?;
        self.heap_string_value(&text)
    }

    fn instant_argument(&mut self, value: Option<&Value>) -> Result<Instant> {
        let value = value.ok_or_else(|| Error::type_error("Instant requires an argument"))?;
        if let Value::Object(id) = value {
            match self.objects.temporal_value(*id)? {
                Some(TemporalValue::Instant(instant)) => return Ok(*instant),
                Some(TemporalValue::ZonedDateTime(zoned)) => return Ok(zoned.to_instant()),
                _ => {}
            }
        }
        let text = if let Some(text) = value.string_text() {
            text.to_owned()
        } else if Self::instant_object(value) {
            self.to_string(value)?
        } else {
            return Err(Error::type_error(
                "Temporal.Instant input must be a string or object",
            ));
        };
        Instant::from_utf8(text.as_bytes()).map_err(temporal_error)
    }

    fn instant_receiver(&self, value: &Value) -> Result<Instant> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(
                "Temporal.Instant method requires an Instant receiver",
            ));
        };
        match self.objects.temporal_value(*id)? {
            Some(TemporalValue::Instant(instant)) => Ok(*instant),
            _ => Err(Error::type_error(
                "Temporal.Instant method requires an Instant receiver",
            )),
        }
    }

    fn create_instant_value(&mut self, instant: Instant) -> Result<Value> {
        self.create_temporal_calendar_value(
            TemporalValue::Instant(instant),
            TemporalFunctionKind::InstantConstructor,
        )
    }

    fn instant_bigint_argument(&mut self, value: Option<&Value>) -> Result<i128> {
        let value = value.cloned().unwrap_or(Value::Undefined);
        let bigint = self.to_bigint(&value)?;
        bigint
            .to_string()
            .parse::<i128>()
            .map_err(|_| Self::instant_range("epochNanoseconds"))
    }

    fn instant_bigint_value(value: i128) -> Result<Value> {
        JsBigInt::parse_string(&value.to_string())
            .map(Value::BigInt)
            .ok_or_else(|| Error::runtime("Instant nanoseconds cannot become BigInt"))
    }

    const fn instant_object(value: &Value) -> bool {
        matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        )
    }

    fn instant_range(field: &str) -> Error {
        Error::exception(
            ErrorName::RangeError,
            format!("Temporal.Instant {field} is out of range"),
        )
    }
}
