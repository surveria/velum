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

const DATE_TIME_COMPONENT_OPTIONS: &[(&str, &[&str])] = &[
    ("weekday", &["long", "short", "narrow"]),
    ("era", &["long", "short", "narrow"]),
    ("year", &["numeric", "2-digit"]),
    ("month", &["numeric", "2-digit", "long", "short", "narrow"]),
    ("day", &["numeric", "2-digit"]),
    ("dayPeriod", &["long", "short", "narrow"]),
    ("hour", &["numeric", "2-digit"]),
    ("minute", &["numeric", "2-digit"]),
    ("second", &["numeric", "2-digit"]),
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
];

struct DateTimePreferences {
    calendar: Option<String>,
    numbering_system: Option<String>,
    hour12: Option<bool>,
    hour_cycle: Option<String>,
    time_zone: String,
}

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
        let preferences = self.parse_date_time_preferences(&options_value)?;
        let mut options = self.parse_date_time_components(
            &options_value,
            preferences.hour12,
            preferences.hour_cycle.clone(),
        )?;
        let (locale, calendar, numbering_system) = resolve_date_time_locale(
            &locale,
            preferences.calendar.as_deref(),
            preferences.numbering_system.as_deref(),
            options.hour12,
            options.hour_cycle.as_deref(),
        );
        options.hour_cycle = Some(resolve_hour_cycle(
            &locale,
            options.hour12,
            options.hour_cycle.as_deref(),
        ));
        Ok(DateTimeFormatValue {
            locale,
            calendar,
            numbering_system,
            time_zone: preferences.time_zone,
            options,
            bound_format: None,
        })
    }

    fn parse_date_time_preferences(&mut self, options: &Value) -> Result<DateTimePreferences> {
        let _locale_matcher =
            self.date_time_option_string(options, "localeMatcher", &["lookup", "best fit"])?;
        let calendar = self.date_time_option_string(options, "calendar", &[])?;
        if calendar
            .as_deref()
            .is_some_and(|calendar| !is_unicode_type(calendar))
        {
            return Err(range_error("calendar has an invalid value"));
        }
        let numbering_system = self.date_time_option_string(options, "numberingSystem", &[])?;
        if numbering_system
            .as_deref()
            .is_some_and(|numbering| !is_unicode_type(numbering))
        {
            return Err(range_error("numberingSystem has an invalid value"));
        }
        let hour12_value = self.date_time_option_value(options, "hour12")?;
        let hour12 = (!matches!(hour12_value, Value::Undefined))
            .then(|| to_boolean(self, &hour12_value))
            .transpose()?;
        let hour_cycle =
            self.date_time_option_string(options, "hourCycle", &["h11", "h12", "h23", "h24"])?;
        let time_zone_value = self.date_time_option_string(options, "timeZone", &[])?;
        let time_zone = resolve_time_zone(time_zone_value.as_deref())?;
        Ok(DateTimePreferences {
            calendar,
            numbering_system,
            hour12,
            hour_cycle,
            time_zone,
        })
    }

    fn parse_date_time_components(
        &mut self,
        options_value: &Value,
        hour12: Option<bool>,
        hour_cycle: Option<String>,
    ) -> Result<DateTimeFormatOptions> {
        let mut options = DateTimeFormatOptions {
            hour_cycle,
            hour12,
            ..DateTimeFormatOptions::default()
        };
        for (name, allowed) in DATE_TIME_COMPONENT_OPTIONS {
            if *name == "timeZoneName" {
                continue;
            }
            let value = self.date_time_option_string(options_value, name, allowed)?;
            set_date_time_component(&mut options, name, value)?;
        }
        let fractional = self.date_time_option_value(options_value, "fractionalSecondDigits")?;
        if !matches!(fractional, Value::Undefined) {
            let number = self.to_number(&fractional)?;
            if !number.is_finite() || !(1.0..=3.0).contains(&number) {
                return Err(range_error("fractionalSecondDigits is out of range"));
            }
            options.fractional_second_digits = number.floor().to_u8();
        }
        options.time_zone_name = self.date_time_option_string(
            options_value,
            "timeZoneName",
            &[
                "long",
                "short",
                "shortOffset",
                "longOffset",
                "shortGeneric",
                "longGeneric",
            ],
        )?;
        let _format_matcher =
            self.date_time_option_string(options_value, "formatMatcher", &["basic", "best fit"])?;
        options.date_style = self.date_time_option_string(
            options_value,
            "dateStyle",
            &["full", "long", "medium", "short"],
        )?;
        options.time_style = self.date_time_option_string(
            options_value,
            "timeStyle",
            &["full", "long", "medium", "short"],
        )?;
        if (options.date_style.is_some() || options.time_style.is_some())
            && (options.has_explicit_date_fields() || options.has_explicit_time_fields())
        {
            return Err(Error::type_error(
                "dateStyle and timeStyle cannot be combined with component options",
            ));
        }
        options.default_components = options.date_style.is_none()
            && options.time_style.is_none()
            && !options.has_explicit_date_fields()
            && !options.has_explicit_time_fields();
        if options.default_components {
            options.year = Some("numeric".to_owned());
            options.month = Some("numeric".to_owned());
            options.day = Some("numeric".to_owned());
        }
        Ok(options)
    }

    pub(super) fn intl_locale(&mut self, value: Option<&Value>) -> Result<String> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok("en-US".to_owned());
        };
        if let Some(text) = value.string_text() {
            return super::number_format::canonical_locale(text);
        }
        if !matches!(value, Value::Object(_)) {
            return Err(Error::type_error("Intl locale list is invalid"));
        }
        let length_value = self.get_named(value, "length")?;
        let length = Self::length_to_usize(
            self.to_length(&length_value)?,
            "Intl locale list length exceeded supported range",
        )?;
        let mut first = None;
        for index in 0..length {
            self.step()?;
            let name = index.to_string();
            let lookup = self.property_lookup(&name);
            if !self.has_property_value_with_lookup(value, lookup)? {
                continue;
            }
            let item = self.get_named(value, &name)?;
            if item.string_text().is_none() && !matches!(item, Value::Object(_)) {
                return Err(Error::type_error("Intl locale entry is invalid"));
            }
            let locale = self.to_string(&item)?;
            let locale = super::number_format::canonical_locale(&locale)?;
            if first.is_none() {
                first = Some(locale);
            }
        }
        Ok(first.unwrap_or_else(|| "en-US".to_owned()))
    }

    fn date_time_option_string(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
    ) -> Result<Option<String>> {
        if matches!(options, Value::Undefined) {
            return Ok(None);
        }
        self.intl_option_string(options, name, allowed)
    }

    fn date_time_option_value(&mut self, options: &Value, name: &str) -> Result<Value> {
        if matches!(options, Value::Undefined) {
            return Ok(Value::Undefined);
        }
        self.get_named(options, name)
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

fn resolve_hour_cycle(locale: &str, hour12: Option<bool>, requested: Option<&str>) -> String {
    if let Some(hour12) = hour12 {
        return if hour12 {
            if locale.to_ascii_lowercase().starts_with("ja") {
                "h11"
            } else {
                "h12"
            }
        } else {
            "h23"
        }
        .to_owned();
    }
    if let Some(requested) = requested {
        return requested.to_owned();
    }
    if let Some(extension) = unicode_extension(locale, "hc")
        && matches!(extension.as_str(), "h11" | "h12" | "h23" | "h24")
    {
        return extension;
    }
    if ["de", "fr", "it", "ja"]
        .iter()
        .any(|prefix| locale.to_ascii_lowercase().starts_with(prefix))
    {
        "h23".to_owned()
    } else {
        "h12".to_owned()
    }
}

pub(super) fn unicode_extension(locale: &str, key: &str) -> Option<String> {
    let lower = locale.to_ascii_lowercase();
    let parts = lower.split('-').collect::<Vec<_>>();
    let extension = parts.iter().position(|part| *part == "u")?;
    let mut index = extension.checked_add(1)?;
    while index < parts.len() {
        let part = *parts.get(index)?;
        if part.len() != 2 {
            index = index.checked_add(1)?;
            continue;
        }
        let value_start = index.checked_add(1)?;
        let mut value_end = value_start;
        while value_end < parts.len() && parts.get(value_end)?.len() >= 3 {
            value_end = value_end.checked_add(1)?;
        }
        if part == key && value_start < value_end {
            return Some(parts.get(value_start..value_end)?.join("-"));
        }
        index = value_end;
    }
    None
}

fn set_date_time_component(
    options: &mut DateTimeFormatOptions,
    name: &str,
    value: Option<String>,
) -> Result<()> {
    match name {
        "weekday" => options.weekday = value,
        "era" => options.era = value,
        "year" => options.year = value,
        "month" => options.month = value,
        "day" => options.day = value,
        "dayPeriod" => options.day_period = value,
        "hour" => options.hour = value,
        "minute" => options.minute = value,
        "second" => options.second = value,
        _ => return Err(Error::runtime("Intl option table is inconsistent")),
    }
    Ok(())
}

fn is_unicode_type(value: &str) -> bool {
    value.split('-').all(|part| {
        (3..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

fn resolve_time_zone(requested: Option<&str>) -> Result<String> {
    let text = requested.unwrap_or("UTC");
    let time_zone = temporal_rs::TimeZone::try_from_identifier_str(text)
        .map_err(|error| Error::exception(ErrorName::RangeError, error.to_string()))?;
    if let Some((canonical_case, _)) = jiff_tzdb::get(text) {
        return Ok(canonical_case.to_owned());
    }
    time_zone
        .identifier()
        .map_err(|error| Error::exception(ErrorName::RangeError, error.to_string()))
}

fn resolve_date_time_locale(
    locale: &str,
    calendar_option: Option<&str>,
    numbering_option: Option<&str>,
    hour12: Option<bool>,
    hour_cycle_option: Option<&str>,
) -> (String, String, String) {
    let base = locale.split("-u-").next().unwrap_or(locale).to_owned();
    let calendar_extension =
        unicode_extension(locale, "ca").and_then(|value| resolve_calendar(&value));
    let calendar_explicit = calendar_option.and_then(resolve_calendar);
    let calendar = calendar_explicit
        .clone()
        .or_else(|| calendar_extension.clone())
        .unwrap_or_else(|| "gregory".to_owned());
    let numbering_extension = unicode_extension(locale, "nu")
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| super::number_digits::digits(value).is_some());
    let numbering_explicit = numbering_option
        .map(str::to_ascii_lowercase)
        .filter(|value| super::number_digits::digits(value).is_some());
    let numbering_system = numbering_explicit
        .clone()
        .or_else(|| numbering_extension.clone())
        .unwrap_or_else(|| {
            if base.to_ascii_lowercase().starts_with("ar") {
                "arab".to_owned()
            } else {
                "latn".to_owned()
            }
        });
    let hour_cycle_extension = unicode_extension(locale, "hc")
        .filter(|value| matches!(value.as_str(), "h11" | "h12" | "h23" | "h24"));
    let mut extensions = Vec::new();
    if calendar_extension.as_ref() == Some(&calendar)
        && calendar_explicit
            .as_ref()
            .is_none_or(|explicit| explicit == &calendar)
    {
        extensions.push(("ca", calendar.clone()));
    }
    if hour12.is_none()
        && hour_cycle_extension
            .as_deref()
            .is_some_and(|extension| hour_cycle_option.is_none_or(|explicit| explicit == extension))
        && let Some(extension) = hour_cycle_extension
    {
        extensions.push(("hc", extension));
    }
    if numbering_extension.as_ref() == Some(&numbering_system)
        && numbering_explicit
            .as_ref()
            .is_none_or(|explicit| explicit == &numbering_system)
    {
        extensions.push(("nu", numbering_system.clone()));
    }
    let resolved_locale = if extensions.is_empty() {
        base
    } else {
        let extension = extensions
            .into_iter()
            .map(|(key, value)| format!("{key}-{value}"))
            .collect::<Vec<_>>()
            .join("-");
        format!("{base}-u-{extension}")
    };
    (resolved_locale, calendar, numbering_system)
}

fn resolve_calendar(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    if matches!(lower.as_str(), "islamic" | "islamic-rgsa") {
        return Some("islamic-civil".to_owned());
    }
    temporal_rs::Calendar::from_str(&lower)
        .ok()
        .map(|calendar| calendar.identifier().to_owned())
}

fn range_error(message: &str) -> Error {
    Error::exception(ErrorName::RangeError, message)
}
