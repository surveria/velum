use num_traits::ToPrimitive;

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, object::NumberFormatValue},
    value::{ErrorName, Value},
};

const ROUNDING_INCREMENTS: &[u16] = &[
    1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000,
];

struct NumberUnitOptions {
    style: String,
    currency: Option<String>,
    currency_display: String,
    currency_sign: String,
    unit: Option<String>,
    unit_display: String,
}

struct NumberFractionOptions {
    minimum_integer: u8,
    minimum_fraction: u8,
    maximum_fraction: u8,
}

struct NumberSignificantOptions {
    minimum: Option<u8>,
    maximum: Option<u8>,
}

struct NumberRoundingOptions {
    increment: u16,
    mode: String,
    priority: String,
    trailing_zero_display: String,
}

impl Context {
    pub(super) fn parse_number_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<NumberFormatValue> {
        let locale = self.number_format_locale(args.as_slice().first())?;
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options, Value::Null) {
            return Err(Error::type_error(
                "Intl.NumberFormat options cannot be null",
            ));
        }
        let (locale, numbering_system) = self.parse_number_locale_options(&options, &locale)?;
        let units = self.parse_number_unit_options(&options)?;
        let notation = self.number_string_option(
            &options,
            "notation",
            &["standard", "scientific", "engineering", "compact"],
            "standard",
        )?;
        let fraction = self.parse_number_fraction_options(&options, &units, &notation)?;
        let significant = self.parse_number_significant_options(&options)?;
        let rounding = self.parse_number_rounding_options(&options, &fraction, &significant)?;
        let compact_display =
            self.number_string_option(&options, "compactDisplay", &["short", "long"], "short")?;
        let use_grouping = self.number_use_grouping(&options, &notation)?;
        let sign_display = self.number_string_option(
            &options,
            "signDisplay",
            &["auto", "never", "always", "exceptZero", "negative"],
            "auto",
        )?;
        Ok(NumberFormatValue {
            locale,
            numbering_system,
            style: units.style,
            currency: units.currency,
            currency_display: units.currency_display,
            currency_sign: units.currency_sign,
            unit: units.unit,
            unit_display: units.unit_display,
            minimum_integer_digits: fraction.minimum_integer,
            minimum_fraction_digits: fraction.minimum_fraction,
            maximum_fraction_digits: fraction.maximum_fraction,
            minimum_significant_digits: significant.minimum,
            maximum_significant_digits: significant.maximum,
            use_grouping,
            notation,
            compact_display,
            sign_display,
            rounding_increment: rounding.increment,
            rounding_mode: rounding.mode,
            rounding_priority: rounding.priority,
            trailing_zero_display: rounding.trailing_zero_display,
            bound_format: None,
        })
    }

    fn parse_number_locale_options(
        &mut self,
        options: &Value,
        locale: &str,
    ) -> Result<(String, String)> {
        let _locale_matcher = self.number_string_option(
            options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let numbering_system_option = self.number_optional_string(options, "numberingSystem")?;
        if let Some(numbering_system) = &numbering_system_option
            && !is_unicode_type(numbering_system)
        {
            return Err(range_error("numberingSystem has an invalid value"));
        }
        Ok(resolved_numbering_system(locale, numbering_system_option))
    }

    fn parse_number_unit_options(&mut self, options: &Value) -> Result<NumberUnitOptions> {
        let style = self.number_string_option(
            options,
            "style",
            &["decimal", "percent", "currency", "unit"],
            "decimal",
        )?;
        let currency_option = self.number_optional_string(options, "currency")?;
        if let Some(currency) = &currency_option
            && !is_currency_code(currency)
        {
            return Err(range_error("currency has an invalid value"));
        }
        let currency_display = self.number_string_option(
            options,
            "currencyDisplay",
            &["code", "symbol", "narrowSymbol", "name"],
            "symbol",
        )?;
        let currency_sign = self.number_string_option(
            options,
            "currencySign",
            &["standard", "accounting"],
            "standard",
        )?;
        if style == "currency" && currency_option.is_none() {
            return Err(Error::type_error("currency style requires currency"));
        }
        let unit_option = self.number_optional_string(options, "unit")?;
        if let Some(unit) = &unit_option
            && !is_sanctioned_unit(unit)
        {
            return Err(range_error("unit has an invalid value"));
        }
        let unit_display = self.number_string_option(
            options,
            "unitDisplay",
            &["short", "narrow", "long"],
            "short",
        )?;
        let currency = match style.as_str() {
            "currency" => currency_option.map(|value| value.to_ascii_uppercase()),
            _ => None,
        };
        let unit = match style.as_str() {
            "unit" => {
                Some(unit_option.ok_or_else(|| Error::type_error("unit style requires unit"))?)
            }
            _ => None,
        };
        Ok(NumberUnitOptions {
            style,
            currency,
            currency_display,
            currency_sign,
            unit,
            unit_display,
        })
    }

    fn parse_number_fraction_options(
        &mut self,
        options: &Value,
        units: &NumberUnitOptions,
        notation: &str,
    ) -> Result<NumberFractionOptions> {
        let minimum_integer = self.number_u8_option(options, "minimumIntegerDigits", 1, 21, 1)?;
        let currency_digits = units.currency.as_deref().map_or(2, currency_minor_digits);
        let default_minimum = if units.style == "currency" && notation == "standard" {
            currency_digits
        } else {
            0
        };
        let default_maximum = if units.style == "currency" && notation == "standard" {
            currency_digits
        } else if units.style == "percent" || notation == "compact" {
            0
        } else {
            3
        };
        let minimum_value = self.number_option_value(options, "minimumFractionDigits")?;
        let maximum_value = self.number_option_value(options, "maximumFractionDigits")?;
        let minimum_present = !matches!(minimum_value, Value::Undefined);
        let maximum_present = !matches!(maximum_value, Value::Undefined);
        let (minimum_fraction, maximum_fraction) = match (minimum_present, maximum_present) {
            (true, _) => {
                let minimum = self.number_u8_value(
                    &minimum_value,
                    "minimumFractionDigits",
                    0,
                    100,
                    default_minimum,
                )?;
                let maximum = self.number_u8_value(
                    &maximum_value,
                    "maximumFractionDigits",
                    minimum,
                    100,
                    default_maximum.max(minimum),
                )?;
                (minimum, maximum)
            }
            (false, true) => {
                let maximum = self.number_u8_value(
                    &maximum_value,
                    "maximumFractionDigits",
                    0,
                    100,
                    default_maximum,
                )?;
                (default_minimum.min(maximum), maximum)
            }
            (false, false) => (default_minimum, default_maximum),
        };
        Ok(NumberFractionOptions {
            minimum_integer,
            minimum_fraction,
            maximum_fraction,
        })
    }

    fn parse_number_significant_options(
        &mut self,
        options: &Value,
    ) -> Result<NumberSignificantOptions> {
        let minimum_value = self.number_option_value(options, "minimumSignificantDigits")?;
        let maximum_value = self.number_option_value(options, "maximumSignificantDigits")?;
        let present = !matches!(minimum_value, Value::Undefined)
            || !matches!(maximum_value, Value::Undefined);
        let minimum = present
            .then(|| self.number_u8_value(&minimum_value, "minimumSignificantDigits", 1, 21, 1))
            .transpose()?;
        let maximum = present
            .then(|| {
                self.number_u8_value(
                    &maximum_value,
                    "maximumSignificantDigits",
                    minimum.unwrap_or(1),
                    21,
                    21,
                )
            })
            .transpose()?;
        Ok(NumberSignificantOptions { minimum, maximum })
    }

    fn parse_number_rounding_options(
        &mut self,
        options: &Value,
        fraction: &NumberFractionOptions,
        significant: &NumberSignificantOptions,
    ) -> Result<NumberRoundingOptions> {
        let increment = self.number_u16_option(options, "roundingIncrement", 1)?;
        if !ROUNDING_INCREMENTS.contains(&increment) {
            return Err(range_error("roundingIncrement has an invalid value"));
        }
        let mode = self.number_string_option(
            options,
            "roundingMode",
            &[
                "ceil",
                "floor",
                "expand",
                "trunc",
                "halfCeil",
                "halfFloor",
                "halfExpand",
                "halfTrunc",
                "halfEven",
            ],
            "halfExpand",
        )?;
        let priority = self.number_string_option(
            options,
            "roundingPriority",
            &["auto", "morePrecision", "lessPrecision"],
            "auto",
        )?;
        let trailing_zero_display = self.number_string_option(
            options,
            "trailingZeroDisplay",
            &["auto", "stripIfInteger"],
            "auto",
        )?;
        if increment != 1 && (priority != "auto" || significant.minimum.is_some()) {
            return Err(Error::type_error(
                "roundingIncrement is incompatible with digit options",
            ));
        }
        if increment != 1 && fraction.minimum_fraction != fraction.maximum_fraction {
            return Err(range_error(
                "roundingIncrement requires equal fraction digit limits",
            ));
        }
        Ok(NumberRoundingOptions {
            increment,
            mode,
            priority,
            trailing_zero_display,
        })
    }

    fn number_format_locale(&mut self, value: Option<&Value>) -> Result<String> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok("en-US".to_owned());
        };
        if matches!(value, Value::Null) {
            return Err(Error::type_error("Intl locale list cannot be null"));
        }
        if let Some(text) = value.string_text() {
            return resolve_number_format_locale(text);
        }
        let Value::Object(_) = value else {
            return Ok("en-US".to_owned());
        };
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
            let locale = resolve_number_format_locale(&locale)?;
            if first.is_none() {
                first = Some(locale);
            }
        }
        Ok(first.unwrap_or_else(|| "en-US".to_owned()))
    }

    fn number_optional_string(&mut self, options: &Value, name: &str) -> Result<Option<String>> {
        if matches!(options, Value::Undefined) {
            return Ok(None);
        }
        self.intl_option_string(options, name, &[])
    }

    fn number_option_value(&mut self, options: &Value, name: &str) -> Result<Value> {
        if matches!(options, Value::Undefined) {
            return Ok(Value::Undefined);
        }
        self.get_named(options, name)
    }

    fn number_string_option(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
        default: &str,
    ) -> Result<String> {
        Ok(self
            .number_optional_string(options, name)?
            .unwrap_or_else(|| default.to_owned()))
        .and_then(|value| {
            if allowed.contains(&value.as_str()) {
                Ok(value)
            } else {
                Err(range_error(&format!("{name} has an unsupported value")))
            }
        })
    }

    fn number_u8_option(
        &mut self,
        options: &Value,
        name: &str,
        minimum: u8,
        maximum: u8,
        default: u8,
    ) -> Result<u8> {
        let value = if matches!(options, Value::Undefined) {
            Value::Undefined
        } else {
            self.get_named(options, name)?
        };
        self.number_u8_value(&value, name, minimum, maximum, default)
    }

    fn number_u8_value(
        &mut self,
        value: &Value,
        name: &str,
        minimum: u8,
        maximum: u8,
        default: u8,
    ) -> Result<u8> {
        if matches!(value, Value::Undefined) {
            return Ok(default);
        }
        let number = self.to_number(value)?.floor();
        let result = number
            .to_u8()
            .filter(|result| (*result >= minimum) && (*result <= maximum))
            .ok_or_else(|| range_error(&format!("{name} is out of range")))?;
        Ok(result)
    }

    fn number_u16_option(&mut self, options: &Value, name: &str, default: u16) -> Result<u16> {
        let value = if matches!(options, Value::Undefined) {
            Value::Undefined
        } else {
            self.get_named(options, name)?
        };
        if matches!(value, Value::Undefined) {
            return Ok(default);
        }
        let number = self.to_number(&value)?;
        if !number.is_finite() || number.fract() != 0.0 {
            return Err(range_error(&format!("{name} is out of range")));
        }
        number
            .to_u16()
            .ok_or_else(|| range_error(&format!("{name} is out of range")))
    }

    fn number_use_grouping(&mut self, options: &Value, notation: &str) -> Result<Option<String>> {
        let value = if matches!(options, Value::Undefined) {
            Value::Undefined
        } else {
            self.get_named(options, "useGrouping")?
        };
        let default = if notation == "compact" {
            "min2"
        } else {
            "auto"
        };
        match value {
            Value::Undefined => Ok(Some(default.to_owned())),
            Value::Bool(false) | Value::Null => Ok(None),
            Value::Bool(true) => Ok(Some("always".to_owned())),
            _ => {
                let text = self.to_string(&value)?;
                match text.as_str() {
                    "auto" | "always" | "min2" => Ok(Some(text)),
                    "true" | "false" => Ok(Some(default.to_owned())),
                    "0" | "" => Ok(None),
                    _ => Err(range_error("useGrouping has an unsupported value")),
                }
            }
        }
    }
}

fn range_error(message: &str) -> Error {
    Error::exception(ErrorName::RangeError, message)
}

pub(super) fn is_unicode_type(value: &str) -> bool {
    value.split('-').all(|part| {
        (3..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

pub(super) fn resolved_numbering_system(locale: &str, option: Option<String>) -> (String, String) {
    let base = locale.split("-u-").next().unwrap_or(locale).to_owned();
    let extension = super::options::unicode_extension(locale, "nu")
        .filter(|value| super::number_digits::digits(value).is_some());
    let explicit = option
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| super::number_digits::digits(value).is_some());
    if let Some(explicit) = explicit {
        let resolved_locale = if extension.as_deref() == Some(explicit.as_str()) {
            format!("{base}-u-nu-{explicit}")
        } else {
            base
        };
        return (resolved_locale, explicit);
    }
    if let Some(extension) = extension {
        return (format!("{base}-u-nu-{extension}"), extension);
    }
    (base, "latn".to_owned())
}

fn resolve_number_format_locale(locale: &str) -> Result<String> {
    let canonical = super::number_format::canonical_locale(locale)?;
    let base = canonical.split("-u-").next().unwrap_or(&canonical);
    let resolved_base = if base.eq_ignore_ascii_case("en-us") {
        "en-US".to_owned()
    } else {
        base.to_owned()
    };
    let extension = super::options::unicode_extension(&canonical, "nu")
        .filter(|value| super::number_digits::digits(value).is_some());
    Ok(extension.map_or_else(
        || resolved_base.clone(),
        |extension| format!("{resolved_base}-u-nu-{extension}"),
    ))
}

fn is_currency_code(value: &str) -> bool {
    value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn currency_minor_digits(currency: &str) -> u8 {
    match currency {
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        "CLF" | "UYW" => 4,
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        _ => 2,
    }
}

fn is_sanctioned_unit(value: &str) -> bool {
    let mut parts = value.split("-per-");
    let Some(numerator) = parts.next() else {
        return false;
    };
    let denominator = parts.next();
    if parts.next().is_some() {
        return false;
    }
    is_simple_unit(numerator) && denominator.is_none_or(is_simple_unit)
}

fn is_simple_unit(value: &str) -> bool {
    matches!(
        value,
        "acre"
            | "bit"
            | "byte"
            | "celsius"
            | "centimeter"
            | "day"
            | "degree"
            | "fahrenheit"
            | "fluid-ounce"
            | "foot"
            | "gallon"
            | "gigabit"
            | "gigabyte"
            | "gram"
            | "hectare"
            | "hour"
            | "inch"
            | "kilobit"
            | "kilobyte"
            | "kilogram"
            | "kilometer"
            | "liter"
            | "megabit"
            | "megabyte"
            | "meter"
            | "microsecond"
            | "mile"
            | "mile-scandinavian"
            | "milliliter"
            | "millimeter"
            | "millisecond"
            | "minute"
            | "month"
            | "nanosecond"
            | "ounce"
            | "percent"
            | "petabyte"
            | "pound"
            | "second"
            | "stone"
            | "terabit"
            | "terabyte"
            | "week"
            | "yard"
            | "year"
    )
}
