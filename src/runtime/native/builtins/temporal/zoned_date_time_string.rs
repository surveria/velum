#[cfg(not(feature = "std"))]
use crate::prelude::*;

use core::str::FromStr;

use num_traits::ToPrimitive;
use temporal_rs::{
    options::{DisplayCalendar, DisplayOffset, DisplayTimeZone, ToStringRoundingOptions},
    parsers::Precision,
};

use crate::{
    error::{Error, Result},
    runtime::Context,
    value::{ErrorName, Value},
};

pub(super) struct ZonedStringOptions {
    pub(super) calendar: DisplayCalendar,
    pub(super) offset: DisplayOffset,
    pub(super) time_zone: DisplayTimeZone,
    pub(super) rounding: ToStringRoundingOptions,
}

enum FractionalDigits {
    Auto,
    Number(f64),
    String(String),
}

impl Context {
    pub(super) fn zoned_string_options(
        &mut self,
        value: Option<&Value>,
    ) -> Result<ZonedStringOptions> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(Self::default_zoned_string_options());
        };
        if !Self::zoned_string_object(value) {
            return Err(Error::type_error("Temporal options must be an object"));
        }

        let calendar = self.zoned_string_option(value, "calendarName")?;
        let fractional = self.zoned_fractional_digits(value)?;
        let offset = self.zoned_string_option(value, "offset")?;
        let rounding_mode = self.zoned_string_option(value, "roundingMode")?;
        let smallest_unit = self.zoned_string_option(value, "smallestUnit")?;
        let time_zone = self.zoned_string_option(value, "timeZoneName")?;

        Ok(ZonedStringOptions {
            calendar: Self::parse_zoned_option(calendar, DisplayCalendar::Auto)?,
            offset: Self::parse_zoned_option(offset, DisplayOffset::Auto)?,
            time_zone: Self::parse_zoned_option(time_zone, DisplayTimeZone::Auto)?,
            rounding: ToStringRoundingOptions {
                precision: Self::parse_fractional_digits(fractional)?,
                smallest_unit: Self::parse_zoned_optional(smallest_unit)?,
                rounding_mode: Self::parse_zoned_optional(rounding_mode)?,
            },
        })
    }

    fn zoned_string_option(&mut self, value: &Value, name: &str) -> Result<Option<String>> {
        let option = self.get_named(value, name)?;
        if matches!(option, Value::Undefined) {
            return Ok(None);
        }
        self.to_string(&option).map(Some)
    }

    fn zoned_fractional_digits(&mut self, value: &Value) -> Result<FractionalDigits> {
        let option = self.get_named(value, "fractionalSecondDigits")?;
        match option {
            Value::Undefined => Ok(FractionalDigits::Auto),
            Value::Number(number) => Ok(FractionalDigits::Number(number)),
            _ => self.to_string(&option).map(FractionalDigits::String),
        }
    }

    fn parse_fractional_digits(value: FractionalDigits) -> Result<Precision> {
        match value {
            FractionalDigits::Auto => Ok(Precision::Auto),
            FractionalDigits::String(text) if text == "auto" => Ok(Precision::Auto),
            FractionalDigits::Number(number) => {
                let digits = number.floor();
                if !digits.is_finite() || !(0.0..=9.0).contains(&digits) {
                    return Err(Self::zoned_string_range("fractionalSecondDigits"));
                }
                digits
                    .to_u8()
                    .map(Precision::Digit)
                    .ok_or_else(|| Self::zoned_string_range("fractionalSecondDigits"))
            }
            FractionalDigits::String(_) => Err(Self::zoned_string_range("fractionalSecondDigits")),
        }
    }

    fn parse_zoned_option<T: FromStr>(value: Option<String>, default: T) -> Result<T> {
        value.map_or(Ok(default), |text| {
            T::from_str(&text).map_err(|_| Self::zoned_string_range("option"))
        })
    }

    fn parse_zoned_optional<T: FromStr>(value: Option<String>) -> Result<Option<T>> {
        value
            .map(|text| T::from_str(&text).map_err(|_| Self::zoned_string_range("option")))
            .transpose()
    }

    const fn default_zoned_string_options() -> ZonedStringOptions {
        ZonedStringOptions {
            calendar: DisplayCalendar::Auto,
            offset: DisplayOffset::Auto,
            time_zone: DisplayTimeZone::Auto,
            rounding: ToStringRoundingOptions {
                precision: Precision::Auto,
                smallest_unit: None,
                rounding_mode: None,
            },
        }
    }

    const fn zoned_string_object(value: &Value) -> bool {
        matches!(
            value,
            Value::Object(_)
                | Value::Function(_)
                | Value::NativeFunction(_)
                | Value::HostFunction(_)
        )
    }

    fn zoned_string_range(name: &str) -> Error {
        Error::exception(
            ErrorName::RangeError,
            format!("Invalid Temporal.ZonedDateTime toString {name}"),
        )
    }
}
