use crate::{
    error::{Error, Result},
    runtime::{
        Context, abstract_operations::NumericValue, call::RuntimeCallArgs,
        object::NumberFormatValue,
    },
    value::{ErrorName, ObjectId, Value},
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

enum NumberInput {
    Number(f64),
    Integer(String),
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
        let formatted = format_number(&formatter, input)?;
        if parts {
            return self.number_parts_value(formatted.parts, None);
        }
        self.heap_string_value(&formatted.text)
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
        let text = format!("{}–{}", start.text, end.text);
        if !parts {
            return self.heap_string_value(&text);
        }
        let mut values = Vec::new();
        for part in start.parts {
            values.push(self.number_part_value(part, Some("startRange"))?);
        }
        values.push(self.number_part_value(
            NumberPart {
                kind: "literal",
                value: "–".to_owned(),
            },
            Some("shared"),
        )?);
        for part in end.parts {
            values.push(self.number_part_value(part, Some("endRange"))?);
        }
        self.create_array_from_elements(values)
    }

    fn number_format_input(&mut self, value: &Value) -> Result<NumberInput> {
        match self.to_numeric(value)? {
            NumericValue::Number(value) => Ok(NumberInput::Number(value)),
            NumericValue::BigInt(value) => Ok(NumberInput::Integer(value.to_string())),
        }
    }

    fn number_parts_value(
        &mut self,
        parts: Vec<NumberPart>,
        source: Option<&'static str>,
    ) -> Result<Value> {
        let mut values = Vec::with_capacity(parts.len());
        for part in parts {
            values.push(self.number_part_value(part, source)?);
        }
        self.create_array_from_elements(values)
    }

    fn number_part_value(
        &mut self,
        part: NumberPart,
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
        NumberInput::Number(value) => format_f64(formatter, value),
        NumberInput::Integer(value) => format_integer(formatter, &value),
    }
}

fn format_f64(formatter: &NumberFormatValue, input: f64) -> Result<FormattedNumber> {
    if input.is_nan() {
        return special_number(formatter, false, "nan", "NaN");
    }
    let negative = input.is_sign_negative();
    if input.is_infinite() {
        return special_number(formatter, negative, "infinity", "∞");
    }
    let mut value = input.abs();
    if formatter.style == "percent" {
        value *= 100.0;
    }
    let rounded = if let Some(maximum) = formatter.maximum_significant_digits {
        format_significant(
            value,
            formatter.minimum_significant_digits.unwrap_or(1),
            maximum,
        )
    } else {
        format_fraction(
            value,
            formatter.minimum_fraction_digits,
            formatter.maximum_fraction_digits,
            formatter.rounding_increment,
            &formatter.rounding_mode,
            &formatter.trailing_zero_display,
        )?
    };
    format_decimal_text(formatter, &rounded, negative, value == 0.0)
}

fn format_integer(formatter: &NumberFormatValue, input: &str) -> Result<FormattedNumber> {
    let (negative, digits) = input
        .strip_prefix('-')
        .map_or((false, input), |digits| (true, digits));
    let minimum = usize::from(formatter.minimum_integer_digits);
    let padded = if digits.len() < minimum {
        let padding = minimum
            .checked_sub(digits.len())
            .ok_or_else(|| Error::limit("number padding underflowed"))?;
        format!("{}{}", "0".repeat(padding), digits)
    } else {
        digits.to_owned()
    };
    format_decimal_text(
        formatter,
        &padded,
        negative,
        digits.bytes().all(|byte| byte == b'0'),
    )
}

fn format_fraction(
    value: f64,
    minimum: u8,
    maximum: u8,
    increment: u16,
    rounding_mode: &str,
    trailing_zero_display: &str,
) -> Result<String> {
    let precision = usize::from(maximum);
    let exponent = i32::from(maximum);
    let scale = 10_f64.powi(exponent);
    let increment_value = f64::from(increment) / scale;
    let scaled = value / increment_value;
    let rounded = match rounding_mode {
        "ceil" => scaled.ceil(),
        "floor" => scaled.floor(),
        "expand" => scaled.ceil(),
        "trunc" => scaled.floor(),
        "halfEven" => round_half_even(scaled),
        "halfCeil" | "halfFloor" | "halfTrunc" | "halfExpand" => scaled.round(),
        _ => return Err(Error::runtime("unsupported NumberFormat rounding mode")),
    } * increment_value;
    let mut text = format!("{rounded:.precision$}");
    let minimum = if trailing_zero_display == "stripIfInteger" && rounded.fract() == 0.0 {
        0
    } else {
        usize::from(minimum)
    };
    trim_fraction(&mut text, minimum);
    Ok(text)
}

fn format_significant(value: f64, minimum: u8, maximum: u8) -> String {
    if value == 0.0 {
        let zeros = usize::from(minimum.saturating_sub(1));
        return if zeros == 0 {
            "0".to_owned()
        } else {
            format!("0.{}", "0".repeat(zeros))
        };
    }
    let magnitude = value.log10().floor();
    let decimal_places = f64::from(maximum) - 1.0 - magnitude;
    let rounded = if decimal_places >= 0.0 {
        let precision = decimal_places.to_usize().unwrap_or(0);
        format!("{value:.precision$}")
    } else {
        let scale = 10_f64.powf(-decimal_places);
        format!("{:.0}", (value / scale).round() * scale)
    };
    let mut text = rounded;
    let required = significant_decimal_minimum(&text, usize::from(minimum));
    trim_fraction(&mut text, required);
    text
}

fn significant_decimal_minimum(text: &str, minimum: usize) -> usize {
    let integer_digits = text
        .split('.')
        .next()
        .map(|integer| integer.trim_start_matches('0').len())
        .unwrap_or(0);
    minimum.saturating_sub(integer_digits)
}

fn trim_fraction(text: &mut String, minimum: usize) {
    let Some(dot) = text.find('.') else {
        return;
    };
    while text.len().saturating_sub(dot).saturating_sub(1) > minimum && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
}

fn round_half_even(value: f64) -> f64 {
    let floor = value.floor();
    let fraction = value - floor;
    if fraction != 0.5 {
        return value.round();
    }
    if (floor / 2.0).fract() == 0.0 {
        floor
    } else {
        floor + 1.0
    }
}

fn format_decimal_text(
    formatter: &NumberFormatValue,
    rounded: &str,
    negative: bool,
    zero: bool,
) -> Result<FormattedNumber> {
    let mut split = rounded.split('.');
    let integer = split.next().unwrap_or("0");
    let fraction = split.next();
    let grouped = group_integer(integer, formatter);
    let decimal_separator = if formatter.locale.to_ascii_lowercase().starts_with("de") {
        ","
    } else {
        "."
    };
    let mut parts = Vec::new();
    push_sign(&mut parts, formatter, negative, zero);
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
    let text = parts.iter().map(|part| part.value.as_str()).collect();
    Ok(FormattedNumber { text, parts })
}

fn special_number(
    formatter: &NumberFormatValue,
    negative: bool,
    kind: &'static str,
    value: &str,
) -> Result<FormattedNumber> {
    let mut parts = Vec::new();
    push_sign(&mut parts, formatter, negative, false);
    push_style_prefix(&mut parts, formatter);
    parts.push(NumberPart {
        kind,
        value: value.to_owned(),
    });
    push_style_suffix(&mut parts, formatter);
    let text = parts.iter().map(|part| part.value.as_str()).collect();
    Ok(FormattedNumber { text, parts })
}

fn push_sign(
    parts: &mut Vec<NumberPart>,
    formatter: &NumberFormatValue,
    negative: bool,
    zero: bool,
) {
    let sign = match formatter.sign_display.as_str() {
        "never" => None,
        "always" => Some(if negative {
            ("minusSign", "-")
        } else {
            ("plusSign", "+")
        }),
        "exceptZero" if zero => None,
        "exceptZero" => Some(if negative {
            ("minusSign", "-")
        } else {
            ("plusSign", "+")
        }),
        "negative" if negative && !zero => Some(("minusSign", "-")),
        "negative" => None,
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
    if formatter.style != "currency" {
        return;
    }
    let currency = formatter.currency.as_deref().unwrap_or("");
    let value = match formatter.currency_display.as_str() {
        "code" => currency,
        "name" => currency,
        _ => currency_symbol(currency),
    };
    parts.push(NumberPart {
        kind: "currency",
        value: value.to_owned(),
    });
}

fn push_style_suffix(parts: &mut Vec<NumberPart>, formatter: &NumberFormatValue) {
    match formatter.style.as_str() {
        "percent" => parts.push(NumberPart {
            kind: "percentSign",
            value: "%".to_owned(),
        }),
        "unit" => parts.push(NumberPart {
            kind: "unit",
            value: format!(" {}", formatter.unit.as_deref().unwrap_or("")),
        }),
        _ => {}
    }
}

fn currency_symbol(currency: &str) -> &str {
    match currency {
        "CNY" => "CN¥",
        "EUR" => "€",
        "GBP" => "£",
        "INR" => "₹",
        "JPY" => "¥",
        "KRW" => "₩",
        "USD" => "$",
        _ => currency,
    }
}

fn group_integer(integer: &str, formatter: &NumberFormatValue) -> String {
    let Some(grouping) = &formatter.use_grouping else {
        return integer.to_owned();
    };
    if integer.len() <= 3 || (grouping == "min2" && integer.len() <= 4) {
        return integer.to_owned();
    }
    let separator = if formatter.locale.to_ascii_lowercase().starts_with("de") {
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

fn push_grouped_integer(parts: &mut Vec<NumberPart>, integer: &str, formatter: &NumberFormatValue) {
    let separator = if formatter.locale.to_ascii_lowercase().starts_with("de") {
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
    let digits = match numbering_system {
        "arab" => "٠١٢٣٤٥٦٧٨٩",
        "hanidec" => "〇一二三四五六七八九",
        "thai" => "๐๑๒๓๔๕๖๗๘๙",
        _ => return value.to_owned(),
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

trait FloatToUsize {
    fn to_usize(self) -> Option<usize>;
}

impl FloatToUsize for f64 {
    fn to_usize(self) -> Option<usize> {
        if self.is_finite() && self >= 0.0 && self <= 100.0 {
            self.to_string().parse().ok()
        } else {
            None
        }
    }
}
