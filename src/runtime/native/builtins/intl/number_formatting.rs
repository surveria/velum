use crate::{
    error::{Error, Result},
    runtime::{
        Context, abstract_operations::NumericValue, call::RuntimeCallArgs,
        object::NumberFormatValue,
    },
    value::{ErrorName, ObjectId, Value},
};

use super::number_range::{format_range_text, range_separator};
use super::number_rounding::{
    NumberInput, RoundedNumber, parse_number_input, round_fraction, round_standard,
};

#[derive(Clone, Debug)]
struct NumberPart {
    kind: &'static str,
    value: String,
}

#[derive(Clone, Debug)]
struct FormattedNumber {
    text: String,
    parts: Vec<NumberPart>,
}

impl Context {
    pub(super) fn eval_intl_number_format_method(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        parts: bool,
    ) -> Result<Value> {
        let Value::Object(formatter) = this_value else {
            return Err(Error::type_error("Intl.NumberFormat receiver is invalid"));
        };
        self.eval_intl_number_format(args, *formatter, parts)
    }

    pub(super) fn eval_intl_number_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
        formatter_id: ObjectId,
        parts: bool,
    ) -> Result<Value> {
        let formatter = self.number_format_receiver(&Value::Object(formatter_id))?;
        let value = args.as_slice().first().unwrap_or(&Value::Undefined);
        let input = self.number_format_input(value)?;
        let output = format_number(&formatter, input)?;
        if parts {
            return self.number_parts_value(output.parts, None);
        }
        self.heap_string_value(&output.text)
    }

    pub(super) fn eval_intl_number_format_range(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        parts: bool,
    ) -> Result<Value> {
        let formatter = self.number_format_receiver(this_value)?;
        let Some(start_value) = args.as_slice().first() else {
            return Err(Error::type_error("formatRange start is required"));
        };
        let Some(end_value) = args.as_slice().get(1) else {
            return Err(Error::type_error("formatRange end is required"));
        };
        if matches!(start_value, Value::Undefined) || matches!(end_value, Value::Undefined) {
            return Err(Error::type_error("formatRange values cannot be undefined"));
        }
        let start = format_number(&formatter, self.number_format_input(start_value)?)?;
        let end = format_number(&formatter, self.number_format_input(end_value)?)?;
        if start.text == "NaN" || end.text == "NaN" {
            return Err(Error::exception(
                ErrorName::RangeError,
                "formatRange values cannot be NaN",
            ));
        }
        if start.text == end.text {
            let text = format!("~{}", start.text);
            if !parts {
                return self.heap_string_value(&text);
            }
            let mut shared = vec![NumberPart {
                kind: "approximatelySign",
                value: "~".to_owned(),
            }];
            shared.extend(start.parts);
            return self.number_parts_value(shared, Some("shared"));
        }
        let separator = range_separator(&formatter);
        let text = format_range_text(&formatter, &start.text, &end.text, separator);
        if !parts {
            return self.heap_string_value(&text);
        }
        let mut values = Vec::new();
        for part in start.parts {
            values.push(self.number_part_value(&part, Some("startRange"))?);
        }
        values.push(self.number_part_value(
            &NumberPart {
                kind: "literal",
                value: separator.to_owned(),
            },
            Some("shared"),
        )?);
        for part in end.parts {
            values.push(self.number_part_value(&part, Some("endRange"))?);
        }
        self.create_array_from_elements(values)
    }

    fn number_format_input(&mut self, value: &Value) -> Result<NumberInput> {
        if let Some(text) = value.string_text() {
            return parse_number_input(text);
        }
        match self.to_numeric(value)? {
            NumericValue::Number(value) if value.is_nan() => Ok(NumberInput::Nan),
            NumericValue::Number(value) if value.is_infinite() => Ok(NumberInput::Infinity {
                negative: value.is_sign_negative(),
            }),
            NumericValue::Number(value) => parse_number_input(&value.to_string()),
            NumericValue::BigInt(value) => parse_number_input(&value.to_string()),
        }
    }

    pub(in crate::runtime::native) fn eval_number_to_locale_string(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.parse_number_format(args)?;
        let input = self.number_format_input(this_value)?;
        let output = format_number(&formatter, input)?;
        self.heap_string_value(&output.text)
    }

    fn number_parts_value(
        &mut self,
        parts: Vec<NumberPart>,
        source: Option<&'static str>,
    ) -> Result<Value> {
        let mut values = Vec::with_capacity(parts.len());
        for part in parts {
            values.push(self.number_part_value(&part, source)?);
        }
        self.create_array_from_elements(values)
    }

    fn number_part_value(
        &mut self,
        part: &NumberPart,
        source: Option<&'static str>,
    ) -> Result<Value> {
        let kind = self.heap_string_value(part.kind)?;
        let value = self.heap_string_value(&part.value)?;
        let mut fields = vec![("type", kind), ("value", value)];
        if let Some(source) = source {
            fields.push(("source", self.heap_string_value(source)?));
        }
        self.create_intl_data_object(fields)
    }
}

fn format_number(formatter: &NumberFormatValue, input: NumberInput) -> Result<FormattedNumber> {
    match input {
        NumberInput::Nan => Ok(special_number(
            formatter,
            false,
            true,
            "nan",
            if locale_starts_with(formatter, "zh") {
                "非數值"
            } else {
                "NaN"
            },
        )),
        NumberInput::Infinity { negative } => {
            Ok(special_number(formatter, negative, false, "infinity", "∞"))
        }
        NumberInput::Finite(mut input) => {
            if formatter.style == "percent" {
                input.scale_power(2)?;
            }
            if matches!(formatter.notation.as_str(), "engineering" | "scientific") {
                return format_exponential(formatter, input);
            }
            if formatter.notation == "compact" {
                return format_compact(formatter, input);
            }
            let rounded = round_standard(&input, formatter)?;
            Ok(format_rounded_number(formatter, &rounded))
        }
    }
}

fn format_rounded_number(
    formatter: &NumberFormatValue,
    rounded: &RoundedNumber,
) -> FormattedNumber {
    format_decimal_text(formatter, &rounded.text, rounded.negative, rounded.zero)
}

fn format_exponential(
    formatter: &NumberFormatValue,
    mut input: super::number_rounding::DecimalInput,
) -> Result<FormattedNumber> {
    let magnitude = input.magnitude()?;
    let exponent = if formatter.notation == "engineering" {
        magnitude.div_euclid(3).saturating_mul(3)
    } else {
        magnitude
    };
    input.scale_power(exponent.saturating_neg())?;
    let rounded = round_fraction(
        &input,
        formatter.minimum_fraction_digits,
        formatter.maximum_fraction_digits,
        1,
        &formatter.rounding_mode,
        &formatter.trailing_zero_display,
    )?;
    let mut output = format_rounded_number(formatter, &rounded);
    output.parts.push(NumberPart {
        kind: "exponentSeparator",
        value: "E".to_owned(),
    });
    if exponent < 0 {
        output.parts.push(NumberPart {
            kind: "exponentMinusSign",
            value: "-".to_owned(),
        });
    }
    output.parts.push(NumberPart {
        kind: "exponentInteger",
        value: localize_digits(
            &exponent.unsigned_abs().to_string(),
            &formatter.numbering_system,
        ),
    });
    refresh_formatted_text(&mut output);
    Ok(output)
}

fn format_compact(
    formatter: &NumberFormatValue,
    mut input: super::number_rounding::DecimalInput,
) -> Result<FormattedNumber> {
    let magnitude = input.magnitude()?;
    let compact = compact_pattern(formatter, magnitude);
    input.scale_power(compact.exponent.saturating_neg())?;
    let scaled_magnitude = magnitude.saturating_sub(compact.exponent);
    let maximum_fraction = u8::try_from(1_i32.saturating_sub(scaled_magnitude).max(0))
        .map_err(|_| Error::limit("compact fraction digits exceeded supported range"))?;
    let rounded = round_fraction(
        &input,
        0,
        maximum_fraction,
        1,
        &formatter.rounding_mode,
        &formatter.trailing_zero_display,
    )?;
    let mut output = format_rounded_number(formatter, &rounded);
    if let Some(suffix) = compact.suffix {
        if compact.separator {
            output.parts.push(NumberPart {
                kind: "literal",
                value: compact_separator(formatter).to_owned(),
            });
        }
        output.parts.push(NumberPart {
            kind: "compact",
            value: suffix.to_owned(),
        });
    }
    refresh_formatted_text(&mut output);
    Ok(output)
}

struct CompactPattern {
    exponent: i32,
    suffix: Option<&'static str>,
    separator: bool,
}

fn compact_pattern(formatter: &NumberFormatValue, magnitude: i32) -> CompactPattern {
    if locale_starts_with(formatter, "ja") || locale_starts_with(formatter, "zh") {
        if magnitude >= 8 {
            return CompactPattern {
                exponent: 8,
                suffix: Some("億"),
                separator: false,
            };
        }
        if magnitude >= 4 {
            return CompactPattern {
                exponent: 4,
                suffix: Some(if locale_starts_with(formatter, "zh") {
                    "萬"
                } else {
                    "万"
                }),
                separator: false,
            };
        }
        return CompactPattern {
            exponent: 0,
            suffix: None,
            separator: false,
        };
    }
    if locale_starts_with(formatter, "ko") {
        let (exponent, suffix) = if magnitude >= 8 {
            (8, Some("억"))
        } else if magnitude >= 4 {
            (4, Some("만"))
        } else if magnitude >= 3 {
            (3, Some("천"))
        } else {
            (0, None)
        };
        return CompactPattern {
            exponent,
            suffix,
            separator: false,
        };
    }
    if formatter.locale.eq_ignore_ascii_case("en-IN") && magnitude >= 5 {
        return CompactPattern {
            exponent: 5,
            suffix: Some("L"),
            separator: false,
        };
    }
    if magnitude >= 6 {
        let (suffix, separator) = compact_million_pattern(formatter);
        return CompactPattern {
            exponent: 6,
            suffix: Some(suffix),
            separator,
        };
    }
    if magnitude >= 3
        && !(locale_starts_with(formatter, "de") && formatter.compact_display == "short")
    {
        let (suffix, separator) = compact_thousand_pattern(formatter);
        return CompactPattern {
            exponent: 3,
            suffix: Some(suffix),
            separator,
        };
    }
    CompactPattern {
        exponent: 0,
        suffix: None,
        separator: false,
    }
}

fn compact_million_pattern(formatter: &NumberFormatValue) -> (&'static str, bool) {
    if locale_starts_with(formatter, "de") {
        if formatter.compact_display == "long" {
            ("Millionen", true)
        } else {
            ("Mio.", true)
        }
    } else if formatter.compact_display == "long" {
        ("million", true)
    } else {
        ("M", false)
    }
}

fn compact_thousand_pattern(formatter: &NumberFormatValue) -> (&'static str, bool) {
    if locale_starts_with(formatter, "de") {
        ("Tausend", true)
    } else if formatter.compact_display == "long" {
        ("thousand", true)
    } else {
        ("K", false)
    }
}

fn compact_separator(formatter: &NumberFormatValue) -> &'static str {
    if locale_starts_with(formatter, "de") && formatter.compact_display == "short" {
        "\u{00a0}"
    } else {
        " "
    }
}

fn refresh_formatted_text(formatted: &mut FormattedNumber) {
    formatted.text = formatted
        .parts
        .iter()
        .map(|part| part.value.as_str())
        .collect();
}

fn format_decimal_text(
    formatter: &NumberFormatValue,
    rounded: &str,
    negative: bool,
    zero: bool,
) -> FormattedNumber {
    let mut split = rounded.split('.');
    let integer = split.next().unwrap_or("0");
    let fraction = split.next();
    let minimum_integer_digits = usize::from(formatter.minimum_integer_digits);
    let padded = if integer.len() < minimum_integer_digits {
        let padding = minimum_integer_digits.saturating_sub(integer.len());
        format!("{}{}", "0".repeat(padding), integer)
    } else {
        integer.to_owned()
    };
    let grouped = group_integer(&padded, formatter);
    let decimal_separator = if uses_decimal_comma(formatter) {
        ","
    } else {
        "."
    };
    let mut parts = Vec::new();
    push_unit_prefix(&mut parts, formatter);
    let accounting = uses_accounting_parentheses(formatter, negative, zero);
    if accounting {
        parts.push(NumberPart {
            kind: "literal",
            value: "(".to_owned(),
        });
    } else {
        push_sign(&mut parts, formatter, negative, zero);
    }
    push_style_prefix(&mut parts, formatter);
    push_grouped_integer(&mut parts, &grouped, formatter);
    if let Some(fraction) = fraction {
        parts.push(NumberPart {
            kind: "decimal",
            value: decimal_separator.to_owned(),
        });
        parts.push(NumberPart {
            kind: "fraction",
            value: localize_digits(fraction, &formatter.numbering_system),
        });
    }
    push_style_suffix(&mut parts, formatter);
    if accounting {
        parts.push(NumberPart {
            kind: "literal",
            value: ")".to_owned(),
        });
    }
    let text = parts.iter().map(|part| part.value.as_str()).collect();
    FormattedNumber { text, parts }
}

fn special_number(
    formatter: &NumberFormatValue,
    negative: bool,
    zero_like: bool,
    kind: &'static str,
    value: &str,
) -> FormattedNumber {
    let mut parts = Vec::new();
    push_sign(&mut parts, formatter, negative, zero_like);
    push_style_prefix(&mut parts, formatter);
    parts.push(NumberPart {
        kind,
        value: value.to_owned(),
    });
    push_style_suffix(&mut parts, formatter);
    let text = parts.iter().map(|part| part.value.as_str()).collect();
    FormattedNumber { text, parts }
}

fn push_sign(
    parts: &mut Vec<NumberPart>,
    formatter: &NumberFormatValue,
    negative: bool,
    zero: bool,
) {
    let sign = match formatter.sign_display.as_str() {
        "exceptZero" if zero => None,
        "always" | "exceptZero" => Some(if negative {
            ("minusSign", "-")
        } else {
            ("plusSign", "+")
        }),
        "negative" if negative && !zero => Some(("minusSign", "-")),
        "never" | "negative" => None,
        _ if negative => Some(("minusSign", "-")),
        _ => None,
    };
    if let Some((kind, value)) = sign {
        parts.push(NumberPart {
            kind,
            value: value.to_owned(),
        });
    }
}

fn push_style_prefix(parts: &mut Vec<NumberPart>, formatter: &NumberFormatValue) {
    if formatter.style != "currency"
        || locale_starts_with(formatter, "de")
        || locale_starts_with(formatter, "pt")
    {
        return;
    }
    let currency = formatter.currency.as_deref().unwrap_or("");
    let value = match formatter.currency_display.as_str() {
        "code" | "name" => currency,
        _ => currency_symbol(formatter, currency),
    };
    parts.push(NumberPart {
        kind: "currency",
        value: value.to_owned(),
    });
}

fn push_style_suffix(parts: &mut Vec<NumberPart>, formatter: &NumberFormatValue) {
    match formatter.style.as_str() {
        "currency"
            if locale_starts_with(formatter, "de") || locale_starts_with(formatter, "pt") =>
        {
            parts.push(NumberPart {
                kind: "literal",
                value: "\u{00a0}".to_owned(),
            });
            let currency = formatter.currency.as_deref().unwrap_or("");
            let value = match formatter.currency_display.as_str() {
                "code" | "name" => currency,
                _ => currency_symbol(formatter, currency),
            };
            parts.push(NumberPart {
                kind: "currency",
                value: value.to_owned(),
            });
        }
        "percent" => parts.push(NumberPart {
            kind: "percentSign",
            value: "%".to_owned(),
        }),
        "unit" if formatter.unit.as_deref() == Some("percent") => parts.push(NumberPart {
            kind: "unit",
            value: "%".to_owned(),
        }),
        "unit" => push_unit_suffix(parts, formatter),
        _ => {}
    }
}

fn push_unit_prefix(parts: &mut Vec<NumberPart>, formatter: &NumberFormatValue) {
    if formatter.style != "unit"
        || formatter.unit.as_deref() != Some("kilometer-per-hour")
        || formatter.unit_display != "long"
    {
        return;
    }
    let value = if locale_starts_with(formatter, "ja") {
        Some("時速")
    } else if locale_starts_with(formatter, "ko") {
        Some("시속")
    } else if locale_starts_with(formatter, "zh") {
        Some("每小時")
    } else {
        None
    };
    if let Some(value) = value {
        parts.push(NumberPart {
            kind: "unit",
            value: value.to_owned(),
        });
        parts.push(NumberPart {
            kind: "literal",
            value: " ".to_owned(),
        });
    }
}

fn push_unit_suffix(parts: &mut Vec<NumberPart>, formatter: &NumberFormatValue) {
    let unit = formatter.unit.as_deref().unwrap_or("");
    if unit != "kilometer-per-hour" {
        push_spaced_unit(parts, unit);
        return;
    }
    let (separator, value) = if locale_starts_with(formatter, "de") {
        (
            " ",
            if formatter.unit_display == "long" {
                "Kilometer pro Stunde"
            } else {
                "km/h"
            },
        )
    } else if locale_starts_with(formatter, "ja") {
        match formatter.unit_display.as_str() {
            "narrow" => ("", "km/h"),
            "long" => (" ", "キロメートル"),
            _ => (" ", "km/h"),
        }
    } else if locale_starts_with(formatter, "ko") {
        (
            "",
            if formatter.unit_display == "long" {
                "킬로미터"
            } else {
                "km/h"
            },
        )
    } else if locale_starts_with(formatter, "zh") {
        (
            if formatter.unit_display == "narrow" {
                ""
            } else {
                " "
            },
            if formatter.unit_display == "long" {
                "公里"
            } else {
                "公里/小時"
            },
        )
    } else {
        match formatter.unit_display.as_str() {
            "narrow" => ("", "km/h"),
            "long" => (" ", "kilometers per hour"),
            _ => (" ", "km/h"),
        }
    };
    if !separator.is_empty() {
        parts.push(NumberPart {
            kind: "literal",
            value: separator.to_owned(),
        });
    }
    parts.push(NumberPart {
        kind: "unit",
        value: value.to_owned(),
    });
}

fn push_spaced_unit(parts: &mut Vec<NumberPart>, unit: &str) {
    parts.push(NumberPart {
        kind: "literal",
        value: " ".to_owned(),
    });
    parts.push(NumberPart {
        kind: "unit",
        value: unit.to_owned(),
    });
}

fn currency_symbol<'a>(formatter: &NumberFormatValue, currency: &'a str) -> &'a str {
    match currency {
        "CNY" => "CN¥",
        "EUR" => "€",
        "GBP" => "£",
        "INR" => "₹",
        "JPY" => "¥",
        "KRW" => "₩",
        "USD" if locale_starts_with(formatter, "ko") || locale_starts_with(formatter, "zh") => {
            "US$"
        }
        "USD" => "$",
        _ => currency,
    }
}

fn uses_accounting_parentheses(formatter: &NumberFormatValue, negative: bool, zero: bool) -> bool {
    formatter.style == "currency"
        && formatter.currency_sign == "accounting"
        && !locale_starts_with(formatter, "de")
        && negative
        && match formatter.sign_display.as_str() {
            "never" => false,
            "exceptZero" | "negative" if zero => false,
            _ => true,
        }
}

fn locale_starts_with(formatter: &NumberFormatValue, language: &str) -> bool {
    formatter
        .locale
        .get(..language.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(language))
}

fn uses_decimal_comma(formatter: &NumberFormatValue) -> bool {
    locale_starts_with(formatter, "de") || locale_starts_with(formatter, "pt")
}

fn group_integer(integer: &str, formatter: &NumberFormatValue) -> String {
    let Some(grouping) = &formatter.use_grouping else {
        return integer.to_owned();
    };
    if integer.len() <= 3 || (grouping == "min2" && integer.len() <= 4) {
        return integer.to_owned();
    }
    if formatter.locale.eq_ignore_ascii_case("en-IN") {
        return group_indian_integer(integer);
    }
    let separator = if locale_starts_with(formatter, "pt") {
        '\u{00a0}'
    } else if locale_starts_with(formatter, "de") {
        '.'
    } else {
        ','
    };
    let mut reversed = String::with_capacity(integer.len().saturating_add(integer.len() / 3));
    for (index, character) in integer.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            reversed.push(separator);
        }
        reversed.push(character);
    }
    reversed.chars().rev().collect()
}

fn group_indian_integer(integer: &str) -> String {
    let split = integer.len().saturating_sub(3);
    let Some(prefix) = integer.get(..split) else {
        return integer.to_owned();
    };
    let Some(suffix) = integer.get(split..) else {
        return integer.to_owned();
    };
    let mut reversed = String::with_capacity(prefix.len().saturating_add(prefix.len() / 2));
    for (index, character) in prefix.chars().rev().enumerate() {
        if index > 0 && index % 2 == 0 {
            reversed.push(',');
        }
        reversed.push(character);
    }
    let grouped_prefix: String = reversed.chars().rev().collect();
    format!("{grouped_prefix},{suffix}")
}

fn push_grouped_integer(parts: &mut Vec<NumberPart>, integer: &str, formatter: &NumberFormatValue) {
    let separator = if locale_starts_with(formatter, "pt") {
        '\u{00a0}'
    } else if locale_starts_with(formatter, "de") {
        '.'
    } else {
        ','
    };
    for (index, group) in integer.split(separator).enumerate() {
        if index > 0 {
            parts.push(NumberPart {
                kind: "group",
                value: separator.to_string(),
            });
        }
        parts.push(NumberPart {
            kind: "integer",
            value: localize_digits(group, &formatter.numbering_system),
        });
    }
}

fn localize_digits(value: &str, numbering_system: &str) -> String {
    let Some(digits) = super::number_digits::digits(numbering_system) else {
        return value.to_owned();
    };
    value
        .chars()
        .map(|character| {
            character
                .to_digit(10)
                .and_then(|digit| usize::try_from(digit).ok())
                .and_then(|digit| digits.chars().nth(digit))
                .unwrap_or(character)
        })
        .collect()
}
