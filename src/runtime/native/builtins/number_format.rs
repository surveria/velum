use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::{ErrorName, Value, format_ecmascript_number},
};

const DIGIT_LIMIT: u32 = 100;
const FIXED_THRESHOLD: f64 = 1e21;
const NAN_TEXT: &str = "NaN";
const EXACT_FRACTION_DIGITS: usize = 1100;
const TO_FIXED_RANGE_ERROR: &str =
    "Number.prototype.toFixed fraction digits must be between 0 and 100";
const TO_EXPONENTIAL_RANGE_ERROR: &str =
    "Number.prototype.toExponential fraction digits must be between 0 and 100";
const TO_PRECISION_RANGE_ERROR: &str =
    "Number.prototype.toPrecision precision must be between 1 and 100";

impl Context {
    pub(in crate::runtime::native) fn eval_number_prototype_to_fixed(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let digits = self.number_digit_argument(args.as_slice().first())?;
        if !(0.0..=f64::from(DIGIT_LIMIT)).contains(&digits) {
            return Err(Error::exception(
                ErrorName::RangeError,
                TO_FIXED_RANGE_ERROR,
            ));
        }
        let fraction = Self::count_from_digits(digits);
        let value = self.number_receiver_value(this_value)?;
        if value.is_nan() {
            return self.heap_string_value(NAN_TEXT);
        }
        if !value.is_finite() || value.abs() >= FIXED_THRESHOLD {
            return self.heap_string_value(&format_ecmascript_number(value));
        }
        let text = Self::format_to_fixed(value, fraction);
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_number_prototype_to_exponential(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let argument = args.as_slice().first().cloned();
        let value = self.number_receiver_value(this_value)?;
        let requested = match argument {
            None | Some(Value::Undefined) => None,
            Some(value) => Some(self.number_digit_value(&value)?),
        };
        if value.is_nan() {
            return self.heap_string_value(NAN_TEXT);
        }
        if !value.is_finite() {
            return self.heap_string_value(&format_ecmascript_number(value));
        }
        let fraction = match requested {
            None => None,
            Some(requested) if (0.0..=f64::from(DIGIT_LIMIT)).contains(&requested) => {
                Some(Self::count_from_digits(requested))
            }
            Some(_) => {
                return Err(Error::exception(
                    ErrorName::RangeError,
                    TO_EXPONENTIAL_RANGE_ERROR,
                ));
            }
        };
        let text = Self::format_to_exponential(value, fraction);
        self.heap_string_value(&text)
    }

    pub(in crate::runtime::native) fn eval_number_prototype_to_precision(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let argument = args.as_slice().first().cloned();
        let value = self.number_receiver_value(this_value)?;
        if matches!(argument, None | Some(Value::Undefined)) {
            return self.heap_string_value(&format_ecmascript_number(value));
        }
        let precision = self.number_digit_value(&argument.unwrap_or(Value::Undefined))?;
        if value.is_nan() {
            return self.heap_string_value(NAN_TEXT);
        }
        if !value.is_finite() {
            return self.heap_string_value(&format_ecmascript_number(value));
        }
        if !(1.0..=f64::from(DIGIT_LIMIT)).contains(&precision) {
            return Err(Error::exception(
                ErrorName::RangeError,
                TO_PRECISION_RANGE_ERROR,
            ));
        }
        let text = Self::format_to_precision(value, Self::count_from_digits(precision));
        self.heap_string_value(&text)
    }

    fn number_digit_argument(&mut self, value: Option<&Value>) -> Result<f64> {
        value.map_or(Ok(0.0), |value| self.number_digit_value(value))
    }

    fn number_digit_value(&mut self, value: &Value) -> Result<f64> {
        self.to_integer_or_infinity(value)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    const fn count_from_digits(value: f64) -> usize {
        // `value` is a validated non-negative integer in `0..=100`, so the
        // conversion is exact and cannot truncate or lose a sign.
        value as usize
    }

    /// Round `|value|` to `fraction` decimal places using round-half-up and
    /// render it, mirroring `Number::toFixed`.
    fn format_to_fixed(value: f64, fraction: usize) -> String {
        let negative = value < 0.0;
        let magnitude = value.abs();
        let exact = format!("{magnitude:.EXACT_FRACTION_DIGITS$}");
        let (integer, frac) = exact.split_once('.').unwrap_or((exact.as_str(), ""));
        let mut digits: Vec<u8> = integer
            .bytes()
            .chain(frac.bytes().take(fraction))
            .map(|byte| byte - b'0')
            .collect();
        let round_up = frac
            .as_bytes()
            .get(fraction)
            .is_some_and(|byte| *byte >= b'5');
        if round_up {
            Self::increment_digits(&mut digits, true);
        }
        let point = digits.len() - fraction;
        Self::compose_fixed(&digits, point, negative)
    }

    fn compose_fixed(digits: &[u8], point: usize, negative: bool) -> String {
        let mut out = String::new();
        if negative {
            out.push('-');
        }
        let integer_part = digits.get(..point).unwrap_or(&[]);
        if integer_part.is_empty() {
            out.push('0');
        } else {
            Self::push_digits(&mut out, integer_part);
        }
        let fraction_part = digits.get(point..).unwrap_or(&[]);
        if !fraction_part.is_empty() {
            out.push('.');
            Self::push_digits(&mut out, fraction_part);
        }
        out
    }

    fn format_to_exponential(value: f64, fraction: Option<usize>) -> String {
        let negative = value < 0.0;
        let magnitude = value.abs();
        let (digits, exponent) = fraction.map_or_else(
            || Self::shortest_significant(magnitude),
            |fraction| Self::round_significant(magnitude, fraction + 1),
        );
        Self::compose_exponential(&digits, exponent, negative)
    }

    fn compose_exponential(digits: &[u8], exponent: i32, negative: bool) -> String {
        let mut out = String::new();
        if negative {
            out.push('-');
        }
        out.push(char::from(b'0' + digits.first().copied().unwrap_or(0)));
        if digits.len() > 1 {
            out.push('.');
            Self::push_digits(&mut out, &digits[1..]);
        }
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&exponent.unsigned_abs().to_string());
        out
    }

    fn format_to_precision(value: f64, precision: usize) -> String {
        let negative = value < 0.0;
        let magnitude = value.abs();
        if magnitude == 0.0 {
            return Self::compose_fixed_precision(&vec![0u8; precision], 0, negative);
        }
        let (digits, exponent) = Self::round_significant(magnitude, precision);
        let precision_exp = i32::try_from(precision).unwrap_or(i32::MAX);
        if exponent < -6 || exponent >= precision_exp {
            return Self::compose_exponential(&digits, exponent, negative);
        }
        Self::compose_fixed_precision(&digits, exponent, negative)
    }

    /// Render `digits` (exactly `precision` significant digits) in positional
    /// form given the base-ten `exponent` of the first digit.
    fn compose_fixed_precision(digits: &[u8], exponent: i32, negative: bool) -> String {
        let mut out = String::new();
        if negative {
            out.push('-');
        }
        if exponent < 0 {
            out.push_str("0.");
            for _ in 0..(-exponent - 1) {
                out.push('0');
            }
            Self::push_digits(&mut out, digits);
            return out;
        }
        let integer_len = exponent.unsigned_abs() as usize + 1;
        for index in 0..integer_len {
            out.push(char::from(b'0' + digits.get(index).copied().unwrap_or(0)));
        }
        if integer_len < digits.len() {
            out.push('.');
            Self::push_digits(&mut out, digits.get(integer_len..).unwrap_or(&[]));
        }
        out
    }

    fn push_digits(out: &mut String, digits: &[u8]) {
        for digit in digits {
            out.push(char::from(b'0' + *digit));
        }
    }

    /// Shortest round-trip significant digits of a finite positive `value` and
    /// the base-ten exponent of the first digit.
    fn shortest_significant(value: f64) -> (Vec<u8>, i32) {
        let scientific = format!("{value:e}");
        let (mantissa, exponent) = scientific
            .split_once('e')
            .unwrap_or((scientific.as_str(), "0"));
        let exponent: i32 = exponent.parse().unwrap_or(0);
        let mut digits: Vec<u8> = mantissa
            .bytes()
            .filter(u8::is_ascii_digit)
            .map(|byte| byte - b'0')
            .collect();
        while digits.len() > 1 && digits.last() == Some(&0) {
            digits.pop();
        }
        if digits.is_empty() {
            digits.push(0);
        }
        (digits, exponent)
    }

    /// Exact significant decimal digits of a finite positive `value`, trimmed of
    /// leading and trailing zeros, plus the base-ten exponent of the first
    /// digit. Returns empty digits for zero.
    fn exact_significant(value: f64) -> (Vec<u8>, i32) {
        if value == 0.0 {
            return (Vec::new(), 0);
        }
        let exact = format!("{value:.EXACT_FRACTION_DIGITS$}");
        let (integer, frac) = exact.split_once('.').unwrap_or((exact.as_str(), ""));
        let integer_len = integer.len();
        let full: Vec<u8> = integer
            .bytes()
            .chain(frac.bytes())
            .map(|byte| byte - b'0')
            .collect();
        let Some(first) = full.iter().position(|digit| *digit != 0) else {
            return (Vec::new(), 0);
        };
        let last = full.iter().rposition(|digit| *digit != 0).unwrap_or(first);
        let digits = full.get(first..=last).unwrap_or(&[]).to_vec();
        let exponent = i32::try_from(integer_len)
            .and_then(|len| i32::try_from(first).map(|first| len - 1 - first))
            .unwrap_or(0);
        (digits, exponent)
    }

    /// Round `value` to `count` significant decimal digits (round-half-up),
    /// returning exactly `count` digits and the base-ten exponent of the first.
    fn round_significant(value: f64, count: usize) -> (Vec<u8>, i32) {
        if value == 0.0 || count == 0 {
            return (vec![0u8; count.max(1)], 0);
        }
        let (digits, exponent) = Self::exact_significant(value);
        if digits.len() <= count {
            let mut padded = digits;
            padded.resize(count, 0);
            return (padded, exponent);
        }
        let mut kept: Vec<u8> = digits.get(..count).unwrap_or(&[]).to_vec();
        let round_up = digits.get(count).is_some_and(|digit| *digit >= 5);
        let mut exponent = exponent;
        if round_up && Self::increment_digits(&mut kept, false) {
            kept.truncate(count);
            exponent += 1;
        }
        (kept, exponent)
    }

    /// Increment a big-endian decimal (0-9) digit vector by one. On overflow it
    /// prepends a leading `1` when `grow` is set; otherwise it rewrites the
    /// buffer to `1` followed by zeros and returns `true`.
    fn increment_digits(digits: &mut Vec<u8>, grow: bool) -> bool {
        for digit in digits.iter_mut().rev() {
            if *digit == 9 {
                *digit = 0;
            } else {
                *digit += 1;
                return false;
            }
        }
        if grow {
            digits.insert(0, 1);
        } else if let Some(first) = digits.first_mut() {
            *first = 1;
        }
        true
    }
}
