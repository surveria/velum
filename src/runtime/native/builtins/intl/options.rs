use std::str::FromStr;

use num_traits::ToPrimitive;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        call::RuntimeCallArgs,
        object::{DateTimeFormatOptions, DateTimeFormatValue},
    },
    value::{ErrorName, Value},
};

const STRING_OPTIONS: &[(&str, &[&str])] = &[
    ("dateStyle", &["full", "long", "medium", "short"]),
    ("timeStyle", &["full", "long", "medium", "short"]),
    ("weekday", &["long", "short", "narrow"]),
    ("era", &["long", "short", "narrow"]),
    ("year", &["numeric", "2-digit"]),
    ("month", &["numeric", "2-digit", "long", "short", "narrow"]),
    ("day", &["numeric", "2-digit"]),
    ("hour", &["numeric", "2-digit"]),
    ("minute", &["numeric", "2-digit"]),
    ("second", &["numeric", "2-digit"]),
    ("dayPeriod", &["long", "short", "narrow"]),
    (
        "timeZoneName",
        &[
            "long",
            "short",
            "shortOffset",
            "longOffset",
            "shortGeneric",
            "longGeneric",
        ],
    ),
    ("hourCycle", &["h11", "h12", "h23", "h24"]),
];

impl Context {
    pub(super) fn parse_date_time_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<DateTimeFormatValue> {
        let locale = self.intl_locale(args.as_slice().first())?;
        let options_value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options_value, Value::Null) {
            return Err(Error::type_error(
                "Intl.DateTimeFormat options cannot be null",
            ));
        }
        let mut options = DateTimeFormatOptions::default();
        if !matches!(options_value, Value::Undefined) {
            for (name, allowed) in STRING_OPTIONS {
                let value = self.intl_option_string(&options_value, name, allowed)?;
                match *name {
                    "dateStyle" => options.date_style = value,
                    "timeStyle" => options.time_style = value,
                    "weekday" => options.weekday = value,
                    "era" => options.era = value,
                    "year" => options.year = value,
                    "month" => options.month = value,
                    "day" => options.day = value,
                    "hour" => options.hour = value,
                    "minute" => options.minute = value,
                    "second" => options.second = value,
                    "dayPeriod" => options.day_period = value,
                    "timeZoneName" => options.time_zone_name = value,
                    "hourCycle" => options.hour_cycle = value,
                    _ => return Err(Error::runtime("Intl option table is inconsistent")),
                }
            }
            let fractional = self.get_named(&options_value, "fractionalSecondDigits")?;
            if !matches!(fractional, Value::Undefined) {
                let digits = self.to_number(&fractional)?;
                if !digits.is_finite() || digits.fract() != 0.0 || !(1.0..=3.0).contains(&digits) {
                    return Err(Error::exception(
                        ErrorName::RangeError,
                        "fractionalSecondDigits is out of range",
                    ));
                }
                options.fractional_second_digits = digits.to_u8();
            }
            let hour12 = self.get_named(&options_value, "hour12")?;
            if !matches!(hour12, Value::Undefined) {
                options.hour12 = Some(to_boolean(&hour12));
            }
        }
        if (options.date_style.is_some() || options.time_style.is_some())
            && (options.has_explicit_date_fields() || options.has_explicit_time_fields())
        {
            return Err(Error::type_error(
                "dateStyle and timeStyle cannot be combined with component options",
            ));
        }
        let calendar = self.intl_calendar(&locale, &options_value)?;
        let time_zone = self.intl_time_zone(&options_value)?;
        Ok(DateTimeFormatValue {
            locale,
            calendar,
            time_zone,
            options,
        })
    }

    pub(super) fn intl_locale(&mut self, value: Option<&Value>) -> Result<String> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok("en-US".to_owned());
        };
        let text = if let Some(text) = value.string_text() {
            text.to_owned()
        } else if matches!(value, Value::Object(_)) {
            let first = self.get_named(value, "0")?;
            if matches!(first, Value::Undefined) {
                return Ok("en-US".to_owned());
            }
            self.to_string(&first)?
        } else {
            return Err(Error::type_error("Intl locale list is invalid"));
        };
        if text.is_empty() {
            return Err(Error::exception(
                ErrorName::RangeError,
                "Intl locale is empty",
            ));
        }
        let lower = text.to_ascii_lowercase();
        if lower.starts_with("de") {
            return Ok(if lower.contains("-u-") {
                text
            } else {
                "de-AT".to_owned()
            });
        }
        if lower.starts_with("ja") {
            return Ok(if lower.contains("-u-") {
                text
            } else {
                "ja".to_owned()
            });
        }
        Ok(if lower.contains("-u-") {
            text
        } else if lower == "en-us" {
            "en-US".to_owned()
        } else {
            "en".to_owned()
        })
    }

    fn intl_calendar(&mut self, locale: &str, options: &Value) -> Result<String> {
        let explicit = if matches!(options, Value::Undefined) {
            None
        } else {
            self.intl_option_string(options, "calendar", &[])?
        };
        let requested = explicit.or_else(|| unicode_extension(locale, "ca"));
        let calendar = requested.unwrap_or_else(|| "gregory".to_owned());
        temporal_rs::Calendar::from_str(&calendar)
            .map(|value| value.identifier().to_owned())
            .map_err(|error| Error::exception(ErrorName::RangeError, error.to_string()))
    }

    fn intl_time_zone(&mut self, options: &Value) -> Result<String> {
        let requested = if matches!(options, Value::Undefined) {
            None
        } else {
            self.intl_option_string(options, "timeZone", &[])?
        };
        let text = requested.unwrap_or_else(|| "UTC".to_owned());
        let time_zone = temporal_rs::TimeZone::try_from_identifier_str(&text)
            .map_err(|error| Error::exception(ErrorName::RangeError, error.to_string()))?;
        time_zone
            .identifier()
            .map_err(|error| Error::exception(ErrorName::RangeError, error.to_string()))
    }

    pub(super) fn intl_option_string(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
    ) -> Result<Option<String>> {
        let value = self.get_named(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        let text = self.to_string(&value)?;
        if !allowed.is_empty() && !allowed.contains(&text.as_str()) {
            return Err(Error::exception(
                ErrorName::RangeError,
                format!("{name} has an unsupported value"),
            ));
        }
        Ok(Some(text))
    }
}

pub(super) fn unicode_extension(locale: &str, key: &str) -> Option<String> {
    let marker = format!("-u-{key}-");
    let lower = locale.to_ascii_lowercase();
    let start = lower.find(&marker)?.checked_add(marker.len())?;
    let tail = lower.get(start..)?;
    for value in [
        "islamic-umalqura",
        "islamic-civil",
        "islamic-tbla",
        "iso8601",
        "gregory",
        "japanese",
    ] {
        if tail.starts_with(value) {
            return Some(value.to_owned());
        }
    }
    Some(tail.split('-').next()?.to_owned())
}
