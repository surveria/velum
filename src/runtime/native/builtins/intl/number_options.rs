use num_traits::ToPrimitive;

use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, object::NumberFormatValue},
    value::{ErrorName, Value},
};

const NUMBERING_SYSTEMS: &[&str] = &["arab", "hanidec", "latn", "thai"];
const ROUNDING_INCREMENTS: &[u16] = &[
    1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000,
];

impl Context {
    pub(super) fn parse_number_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<NumberFormatValue> {
        let locale = self.intl_locale(args.as_slice().first())?;
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options, Value::Null) {
            return Err(Error::type_error(
                "Intl.NumberFormat options cannot be null",
            ));
        }

        let _locale_matcher = self.number_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let numbering_system_option = self.number_optional_string(&options, "numberingSystem")?;
        if let Some(numbering_system) = &numbering_system_option
            && !is_unicode_type(numbering_system)
        {
            return Err(range_error("numberingSystem has an invalid value"));
        }
        let numbering_system = resolved_numbering_system(&locale, numbering_system_option);

        let style = self.number_string_option(
            &options,
            "style",
            &["decimal", "percent", "currency", "unit"],
            "decimal",
        )?;
        let currency_option = self.number_optional_string(&options, "currency")?;
        if let Some(currency) = &currency_option
            && !is_currency_code(currency)
        {
            return Err(range_error("currency has an invalid value"));
        }
        let currency_display = self.number_string_option(
            &options,
            "currencyDisplay",
            &["code", "symbol", "narrowSymbol", "name"],
            "symbol",
        )?;
        let currency_sign = self.number_string_option(
            &options,
            "currencySign",
            &["standard", "accounting"],
            "standard",
        )?;
        let unit_option = self.number_optional_string(&options, "unit")?;
        if let Some(unit) = &unit_option
            && !is_sanctioned_unit(unit)
        {
            return Err(range_error("unit has an invalid value"));
        }
        let unit_display = self.number_string_option(
            &options,
            "unitDisplay",
            &["short", "narrow", "long"],
            "short",
        )?;
        let currency = match style.as_str() {
            "currency" => Some(
                currency_option
                    .ok_or_else(|| Error::type_error("currency style requires currency"))?
                    .to_ascii_uppercase(),
            ),
            _ => None,
        };
        let unit = match style.as_str() {
            "unit" => {
                Some(unit_option.ok_or_else(|| Error::type_error("unit style requires unit"))?)
            }
            _ => None,
        };

        let notation = self.number_string_option(
            &options,
            "notation",
            &["standard", "scientific", "engineering", "compact"],
            "standard",
        )?;
        let minimum_integer_digits =
            self.number_u8_option(&options, "minimumIntegerDigits", 1, 21, 1)?;

        let currency_digits = currency.as_deref().map_or(2, currency_minor_digits);
        let default_minimum_fraction = match style.as_str() {
            "currency" => currency_digits,
            _ => 0,
        };
        let default_maximum_fraction = match style.as_str() {
            "currency" => currency_digits,
            "percent" => 0,
            _ => 3,
        };
        let minimum_fraction_value = self.number_option_value(&options, "minimumFractionDigits")?;
        let maximum_fraction_value = self.number_option_value(&options, "maximumFractionDigits")?;
        let minimum_fraction_digits = self.number_u8_value(
            &minimum_fraction_value,
            "minimumFractionDigits",
            0,
            100,
            default_minimum_fraction,
        )?;
        let maximum_fraction_default = default_maximum_fraction.max(minimum_fraction_digits);
        let maximum_fraction_digits = self.number_u8_value(
            &maximum_fraction_value,
            "maximumFractionDigits",
            minimum_fraction_digits,
            100,
            maximum_fraction_default,
        )?;

        let minimum_significant_value =
            self.number_option_value(&options, "minimumSignificantDigits")?;
        let maximum_significant_value =
            self.number_option_value(&options, "maximumSignificantDigits")?;
        let has_significant = !matches!(minimum_significant_value, Value::Undefined)
            || !matches!(maximum_significant_value, Value::Undefined);
        let minimum_significant_digits = has_significant
            .then(|| {
                self.number_u8_value(
                    &minimum_significant_value,
                    "minimumSignificantDigits",
                    1,
                    21,
                    1,
                )
            })
            .transpose()?;
        let maximum_significant_digits = has_significant
            .then(|| {
                self.number_u8_value(
                    &maximum_significant_value,
                    "maximumSignificantDigits",
                    minimum_significant_digits.unwrap_or(1),
                    21,
                    21,
                )
            })
            .transpose()?;

        let rounding_increment = self.number_u16_option(&options, "roundingIncrement", 1)?;
        if !ROUNDING_INCREMENTS.contains(&rounding_increment) {
            return Err(range_error("roundingIncrement has an invalid value"));
        }
        let rounding_mode = self.number_string_option(
            &options,
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
        let rounding_priority = self.number_string_option(
            &options,
            "roundingPriority",
            &["auto", "morePrecision", "lessPrecision"],
            "auto",
        )?;
        let trailing_zero_display = self.number_string_option(
            &options,
            "trailingZeroDisplay",
            &["auto", "stripIfInteger"],
            "auto",
        )?;
        if rounding_increment != 1
            && (rounding_priority != "auto"
                || has_significant
                || minimum_fraction_digits != maximum_fraction_digits)
        {
            return Err(Error::type_error(
                "roundingIncrement is incompatible with digit options",
            ));
        }

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
            style,
            currency,
            currency_display,
            currency_sign,
            unit,
            unit_display,
            minimum_integer_digits,
            minimum_fraction_digits,
            maximum_fraction_digits,
            minimum_significant_digits,
            maximum_significant_digits,
            use_grouping,
            notation,
            compact_display,
            sign_display,
            rounding_increment,
            rounding_mode,
            rounding_priority,
            trailing_zero_display,
            bound_format: None,
        })
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
            .map_or_else(|| default.to_owned(), |value| value))
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
                Ok(match text.as_str() {
                    "auto" | "always" | "min2" => Some(text),
                    _ => Some(default.to_owned()),
                })
            }
        }
    }
}

fn range_error(message: &str) -> Error {
    Error::exception(ErrorName::RangeError, message)
}

fn is_unicode_type(value: &str) -> bool {
    value.split('-').all(|part| {
        (3..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
    })
}

fn resolved_numbering_system(locale: &str, option: Option<String>) -> String {
    let requested = option
        .map(|value| value.to_ascii_lowercase())
        .or_else(|| super::options::unicode_extension(locale, "nu"));
    requested
        .filter(|value| NUMBERING_SYSTEMS.contains(&value.as_str()))
        .unwrap_or_else(|| "latn".to_owned())
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
