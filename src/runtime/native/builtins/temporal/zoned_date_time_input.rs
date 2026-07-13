use std::str::FromStr;

use num_traits::ToPrimitive;
use temporal_rs::{
    Calendar, MonthCode, TimeZone, UtcOffset, ZonedDateTime,
    fields::{CalendarFields, ZonedDateTimeFields},
    options::{Disambiguation, OffsetDisambiguation, Overflow},
    parsed_intermediates::ParsedZonedDateTime,
    partial::{PartialTime, PartialZonedDateTime},
};

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, object::TemporalValue},
    value::{ErrorName, Value},
};

use super::temporal_error;

enum ZonedInput {
    Resolved(ZonedDateTime),
    Parsed(ParsedZonedDateTime),
    Fields(Box<ZonedInputFields>),
}

struct ZonedInputFields {
    calendar: Calendar,
    day: Option<i64>,
    hour: Option<i64>,
    microsecond: Option<i64>,
    millisecond: Option<i64>,
    minute: Option<i64>,
    month: Option<i64>,
    month_code: Option<MonthCode>,
    nanosecond: Option<i64>,
    offset: Option<UtcOffset>,
    second: Option<i64>,
    time_zone: TimeZone,
    year: Option<i64>,
}

#[derive(Clone, Copy)]
struct ZonedOptions {
    disambiguation: Disambiguation,
    offset: OffsetDisambiguation,
    overflow: Overflow,
}

impl Context {
    pub(super) fn zoned_date_time_argument(
        &mut self,
        value: Option<&Value>,
    ) -> Result<ZonedDateTime> {
        self.zoned_date_time_argument_with_options(value, None, OffsetDisambiguation::Reject)
    }

    pub(super) fn zoned_date_time_argument_with_options(
        &mut self,
        value: Option<&Value>,
        options: Option<&Value>,
        default_offset: OffsetDisambiguation,
    ) -> Result<ZonedDateTime> {
        let input = self.prepare_zoned_input(value)?;
        if let ZonedInput::Fields(fields) = &input {
            Self::zoned_calendar_fields(fields, Overflow::Constrain)?;
            Self::zoned_time_fields(fields, Overflow::Constrain)?;
        }
        let options = self.zoned_options(options, default_offset)?;
        match input {
            ZonedInput::Resolved(zoned) => Ok(zoned),
            ZonedInput::Parsed(parsed) => {
                ZonedDateTime::from_parsed(parsed, options.disambiguation, options.offset)
                    .map_err(temporal_error)
            }
            ZonedInput::Fields(fields) => Self::resolve_zoned_fields(*fields, options),
        }
    }

    pub(super) fn eval_zoned_date_time_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        receiver: &Value,
    ) -> Result<Value> {
        let values = args.as_slice();
        let fields_value = values
            .first()
            .ok_or_else(|| Error::type_error("ZonedDateTime.with requires fields"))?;
        if !Self::zoned_input_object(fields_value) {
            return Err(Error::type_error(
                "ZonedDateTime.with fields must be an object",
            ));
        }
        if let Value::Object(id) = fields_value
            && self.objects.temporal_value(*id)?.is_some()
        {
            return Err(Error::type_error(
                "ZonedDateTime.with does not accept Temporal objects",
            ));
        }
        let calendar = self.get_named(fields_value, "calendar")?;
        let time_zone = self.get_named(fields_value, "timeZone")?;
        if !matches!(calendar, Value::Undefined) || !matches!(time_zone, Value::Undefined) {
            return Err(Error::type_error(
                "ZonedDateTime.with fields cannot include calendar or timeZone",
            ));
        }
        let fields = self.prepare_zoned_update_fields(fields_value)?;
        Self::resolve_zoned_update_fields(&fields, Overflow::Constrain)?;
        let options = self.zoned_options(values.get(1), OffsetDisambiguation::Prefer)?;
        let zoned = self.zoned_date_time_receiver(receiver)?;
        let result = zoned
            .with(
                Self::resolve_zoned_update_fields(&fields, options.overflow)?,
                Some(options.disambiguation),
                Some(options.offset),
                Some(options.overflow),
            )
            .map_err(temporal_error)?;
        self.create_zoned_date_time_value(result)
    }

    fn prepare_zoned_input(&mut self, value: Option<&Value>) -> Result<ZonedInput> {
        let value = value
            .ok_or_else(|| Error::type_error("Temporal.ZonedDateTime requires an argument"))?;
        if let Value::Object(id) = value
            && let Some(TemporalValue::ZonedDateTime(zoned)) = self.objects.temporal_value(*id)?
        {
            return Ok(ZonedInput::Resolved(zoned.clone()));
        }
        if let Some(text) = value.string_text() {
            return ParsedZonedDateTime::from_utf8(text.as_bytes())
                .map(ZonedInput::Parsed)
                .map_err(temporal_error);
        }
        if !Self::zoned_input_object(value) {
            return Err(Error::type_error(
                "Temporal.ZonedDateTime input must be a string or object",
            ));
        }
        self.prepare_zoned_fields(value)
            .map(Box::new)
            .map(ZonedInput::Fields)
    }

    fn prepare_zoned_fields(&mut self, value: &Value) -> Result<ZonedInputFields> {
        let calendar_value = self.get_named(value, "calendar")?;
        let calendar = self.temporal_calendar(Some(&calendar_value))?;
        let day = self.zoned_optional_integer(value, "day")?;
        let hour = self.zoned_optional_integer(value, "hour")?;
        let microsecond = self.zoned_optional_integer(value, "microsecond")?;
        let millisecond = self.zoned_optional_integer(value, "millisecond")?;
        let minute = self.zoned_optional_integer(value, "minute")?;
        let month = self.zoned_optional_integer(value, "month")?;
        let month_code = self.zoned_month_code(value)?;
        let nanosecond = self.zoned_optional_integer(value, "nanosecond")?;
        let offset = self.zoned_offset(value)?;
        let second = self.zoned_optional_integer(value, "second")?;
        let time_zone = self.zoned_required_time_zone(value)?;
        let year = self.zoned_optional_integer(value, "year")?;
        Ok(ZonedInputFields {
            calendar,
            day,
            hour,
            microsecond,
            millisecond,
            minute,
            month,
            month_code,
            nanosecond,
            offset,
            second,
            time_zone,
            year,
        })
    }

    fn prepare_zoned_update_fields(&mut self, value: &Value) -> Result<ZonedInputFields> {
        let day = self.zoned_optional_integer(value, "day")?;
        let hour = self.zoned_optional_integer(value, "hour")?;
        let microsecond = self.zoned_optional_integer(value, "microsecond")?;
        let millisecond = self.zoned_optional_integer(value, "millisecond")?;
        let minute = self.zoned_optional_integer(value, "minute")?;
        let month = self.zoned_optional_integer(value, "month")?;
        let month_code = self.zoned_month_code(value)?;
        let nanosecond = self.zoned_optional_integer(value, "nanosecond")?;
        let offset = self.zoned_offset(value)?;
        let second = self.zoned_optional_integer(value, "second")?;
        let year = self.zoned_optional_integer(value, "year")?;
        Ok(ZonedInputFields {
            calendar: Calendar::default(),
            day,
            hour,
            microsecond,
            millisecond,
            minute,
            month,
            month_code,
            nanosecond,
            offset,
            second,
            time_zone: TimeZone::try_from_str("UTC").map_err(temporal_error)?,
            year,
        })
    }

    fn resolve_zoned_fields(
        fields: ZonedInputFields,
        options: ZonedOptions,
    ) -> Result<ZonedDateTime> {
        let calendar_fields = Self::zoned_calendar_fields(&fields, options.overflow)?;
        let time = Self::zoned_time_fields(&fields, options.overflow)?;
        let mut partial = PartialZonedDateTime::new()
            .with_calendar_fields(calendar_fields)
            .with_time(time)
            .with_timezone(Some(fields.time_zone));
        partial.calendar = fields.calendar;
        if let Some(offset) = fields.offset {
            partial = partial.with_offset(offset);
        }
        ZonedDateTime::from_partial(
            partial,
            Some(options.overflow),
            Some(options.disambiguation),
            Some(options.offset),
        )
        .map_err(temporal_error)
    }

    fn resolve_zoned_update_fields(
        fields: &ZonedInputFields,
        overflow: Overflow,
    ) -> Result<ZonedDateTimeFields> {
        Ok(ZonedDateTimeFields {
            calendar_fields: Self::zoned_calendar_fields(fields, overflow)?,
            time: Self::zoned_time_fields(fields, overflow)?,
            offset: fields.offset,
        })
    }

    fn zoned_calendar_fields(
        fields: &ZonedInputFields,
        overflow: Overflow,
    ) -> Result<CalendarFields> {
        Ok(CalendarFields {
            year: Self::zoned_i32(fields.year, "year")?,
            month: Self::zoned_u8(fields.month, "month", overflow)?,
            month_code: fields.month_code,
            day: Self::zoned_u8(fields.day, "day", overflow)?,
            era: None,
            era_year: None,
        })
    }

    fn zoned_time_fields(fields: &ZonedInputFields, overflow: Overflow) -> Result<PartialTime> {
        Ok(PartialTime::new()
            .with_hour(Self::zoned_u8(fields.hour, "hour", overflow)?)
            .with_minute(Self::zoned_u8(fields.minute, "minute", overflow)?)
            .with_second(Self::zoned_u8(fields.second, "second", overflow)?)
            .with_millisecond(Self::zoned_u16(
                fields.millisecond,
                "millisecond",
                overflow,
            )?)
            .with_microsecond(Self::zoned_u16(
                fields.microsecond,
                "microsecond",
                overflow,
            )?)
            .with_nanosecond(Self::zoned_u16(fields.nanosecond, "nanosecond", overflow)?))
    }

    fn zoned_options(
        &mut self,
        value: Option<&Value>,
        default_offset: OffsetDisambiguation,
    ) -> Result<ZonedOptions> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(ZonedOptions {
                disambiguation: Disambiguation::Compatible,
                offset: default_offset,
                overflow: Overflow::Constrain,
            });
        };
        if !Self::zoned_input_object(value) {
            return Err(Error::type_error("Temporal options must be an object"));
        }
        let disambiguation = self.zoned_enum_option(
            value,
            "disambiguation",
            Disambiguation::Compatible,
            Disambiguation::from_str,
        )?;
        let offset = self.zoned_enum_option(
            value,
            "offset",
            default_offset,
            OffsetDisambiguation::from_str,
        )?;
        let overflow =
            self.zoned_enum_option(value, "overflow", Overflow::Constrain, Overflow::from_str)?;
        Ok(ZonedOptions {
            disambiguation,
            offset,
            overflow,
        })
    }

    fn zoned_enum_option<T, E>(
        &mut self,
        value: &Value,
        name: &str,
        default: T,
        parse: fn(&str) -> std::result::Result<T, E>,
    ) -> Result<T> {
        let option = self.get_named(value, name)?;
        if matches!(option, Value::Undefined) {
            return Ok(default);
        }
        let text = self.to_string(&option)?;
        parse(&text).map_err(|_| {
            Error::exception(
                ErrorName::RangeError,
                format!("Invalid Temporal {name}: {text}"),
            )
        })
    }

    fn zoned_optional_integer(&mut self, value: &Value, name: &str) -> Result<Option<i64>> {
        let field = self.get_named(value, name)?;
        if matches!(field, Value::Undefined) {
            return Ok(None);
        }
        let number = self.to_number(&field)?;
        if !number.is_finite() {
            return Err(Self::zoned_field_range(name));
        }
        number
            .trunc()
            .to_i64()
            .map(Some)
            .ok_or_else(|| Self::zoned_field_range(name))
    }

    fn zoned_month_code(&mut self, value: &Value) -> Result<Option<MonthCode>> {
        let field = self.get_named(value, "monthCode")?;
        if matches!(field, Value::Undefined) {
            return Ok(None);
        }
        self.plain_date_month_code(&field).map(Some)
    }

    fn zoned_offset(&mut self, value: &Value) -> Result<Option<UtcOffset>> {
        let field = self.get_named(value, "offset")?;
        if matches!(field, Value::Undefined) {
            return Ok(None);
        }
        let text = if let Some(text) = field.string_text() {
            text.to_owned()
        } else if Self::zoned_input_object(&field) {
            self.to_string(&field)?
        } else {
            return Err(Error::type_error("ZonedDateTime offset must be a string"));
        };
        UtcOffset::from_utf8(text.as_bytes())
            .map(Some)
            .map_err(temporal_error)
    }

    fn zoned_required_time_zone(&mut self, value: &Value) -> Result<TimeZone> {
        let field = self.get_named(value, "timeZone")?;
        if let Value::Object(id) = field
            && let Some(TemporalValue::ZonedDateTime(zoned)) = self.objects.temporal_value(id)?
        {
            return Ok(*zoned.time_zone());
        }
        let text = field.string_text().ok_or_else(|| {
            Error::type_error("ZonedDateTime property bag requires a string timeZone")
        })?;
        TimeZone::try_from_str(text).map_err(temporal_error)
    }

    fn zoned_i32(value: Option<i64>, name: &str) -> Result<Option<i32>> {
        value
            .map(|value| value.to_i32().ok_or_else(|| Self::zoned_field_range(name)))
            .transpose()
    }

    fn zoned_u8(value: Option<i64>, name: &str, overflow: Overflow) -> Result<Option<u8>> {
        value
            .map(|value| {
                if value < 0 {
                    return Err(Self::zoned_field_range(name));
                }
                let normalized = match overflow {
                    Overflow::Constrain => value.min(i64::from(u8::MAX)),
                    Overflow::Reject => value,
                };
                normalized
                    .to_u8()
                    .ok_or_else(|| Self::zoned_field_range(name))
            })
            .transpose()
    }

    fn zoned_u16(value: Option<i64>, name: &str, overflow: Overflow) -> Result<Option<u16>> {
        value
            .map(|value| {
                if value < 0 {
                    return Err(Self::zoned_field_range(name));
                }
                let normalized = match overflow {
                    Overflow::Constrain => value.min(i64::from(u16::MAX)),
                    Overflow::Reject => value,
                };
                normalized
                    .to_u16()
                    .ok_or_else(|| Self::zoned_field_range(name))
            })
            .transpose()
    }

    const fn zoned_input_object(value: &Value) -> bool {
        matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        )
    }

    fn zoned_field_range(name: &str) -> Error {
        Error::exception(
            ErrorName::RangeError,
            format!("Temporal.ZonedDateTime {name} is out of range"),
        )
    }
}
