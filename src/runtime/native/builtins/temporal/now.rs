use std::time::{SystemTime, UNIX_EPOCH};

use temporal_rs::{Instant, TimeZone, ZonedDateTime};

use crate::{
    error::{Error, Result},
    runtime::{
        Context, call::RuntimeCallArgs, native::TemporalFunctionKind, object::TemporalValue,
    },
    value::Value,
};

use super::{temporal_error, temporal_kind};

const NOW_TAG: &str = "Temporal.Now";
const UTC_TIME_ZONE: &str = "UTC";
const METHODS: &[(&str, TemporalFunctionKind)] = &[
    ("instant", TemporalFunctionKind::NowInstant),
    ("timeZoneId", TemporalFunctionKind::NowTimeZoneId),
    (
        "zonedDateTimeISO",
        TemporalFunctionKind::NowZonedDateTimeIso,
    ),
    (
        "plainDateTimeISO",
        TemporalFunctionKind::NowPlainDateTimeIso,
    ),
    ("plainDateISO", TemporalFunctionKind::NowPlainDateIso),
    ("plainTimeISO", TemporalFunctionKind::NowPlainTimeIso),
];

impl Context {
    pub(super) fn temporal_now_value(&mut self) -> Result<Value> {
        self.object_constructor_value()?;
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let object = self.objects.create_with_prototype_id(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        for (name, kind) in METHODS {
            let function = self.create_native_function(temporal_kind(*kind), Value::Undefined)?;
            self.define_non_enumerable_object_property(object, name, function)?;
        }
        self.define_temporal_to_string_tag(object, NOW_TAG)?;
        Ok(Value::Object(object))
    }

    pub(super) fn eval_temporal_now_kind(
        &mut self,
        kind: TemporalFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        match kind {
            TemporalFunctionKind::NowInstant => {
                let instant = Self::temporal_now_instant()?;
                self.create_temporal_calendar_value(
                    TemporalValue::Instant(instant),
                    TemporalFunctionKind::InstantConstructor,
                )
            }
            TemporalFunctionKind::NowTimeZoneId => self.heap_string_value(UTC_TIME_ZONE),
            TemporalFunctionKind::NowZonedDateTimeIso => {
                let zoned = Self::temporal_now_zoned(args.as_slice().first())?;
                self.create_zoned_date_time_value(zoned)
            }
            TemporalFunctionKind::NowPlainDateTimeIso => {
                let date_time =
                    Self::temporal_now_zoned(args.as_slice().first())?.to_plain_date_time();
                self.create_plain_date_time_value(date_time)
            }
            TemporalFunctionKind::NowPlainDateIso => {
                let date = Self::temporal_now_zoned(args.as_slice().first())?.to_plain_date();
                self.create_plain_date_value(date)
            }
            TemporalFunctionKind::NowPlainTimeIso => {
                let time = Self::temporal_now_zoned(args.as_slice().first())?.to_plain_time();
                self.create_temporal_calendar_value(
                    TemporalValue::PlainTime(time),
                    TemporalFunctionKind::PlainTimeConstructor,
                )
            }
            _ => Err(Error::runtime("Temporal.Now function kind was not handled")),
        }
    }

    fn temporal_now_zoned(value: Option<&Value>) -> Result<ZonedDateTime> {
        let time_zone = Self::temporal_now_time_zone(value)?;
        let instant = Self::temporal_now_instant()?;
        instant
            .to_zoned_date_time_iso(time_zone)
            .map_err(temporal_error)
    }

    fn temporal_now_time_zone(value: Option<&Value>) -> Result<TimeZone> {
        let text = match value.filter(|value| !matches!(value, Value::Undefined)) {
            None => UTC_TIME_ZONE,
            Some(value) => value
                .string_text()
                .ok_or_else(|| Error::type_error("Temporal.Now time zone must be a string"))?,
        };
        TimeZone::try_from_str(text).map_err(temporal_error)
    }

    fn temporal_now_instant() -> Result<Instant> {
        let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => i128::try_from(duration.as_nanos())
                .map_err(|_| Error::limit("system time nanoseconds overflowed"))?,
            Err(error) => {
                let magnitude = i128::try_from(error.duration().as_nanos())
                    .map_err(|_| Error::limit("system time nanoseconds overflowed"))?;
                magnitude
                    .checked_neg()
                    .ok_or_else(|| Error::limit("system time nanoseconds overflowed"))?
            }
        };
        Instant::try_new(nanos).map_err(temporal_error)
    }
}
