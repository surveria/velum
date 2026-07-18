use alloc::rc::Rc;
use core::{cmp::Ordering, fmt};

use num_bigint::BigInt;
use num_traits::{FromPrimitive, Signed, ToPrimitive, Zero};

/// Immutable arbitrary-precision ECMAScript `BigInt` primitive.
///
/// `BigInt` has mathematical value semantics and no VM identity. The shared
/// payload keeps cloned bytecode literals and runtime values cheap while
/// equality always compares the represented integer.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JsBigInt(Rc<BigInt>);

impl JsBigInt {
    #[must_use]
    pub fn zero() -> Self {
        Self(Rc::new(BigInt::ZERO))
    }

    #[must_use]
    pub fn from_i64(value: i64) -> Self {
        Self(Rc::new(BigInt::from(value)))
    }

    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        Self(Rc::new(BigInt::from(value)))
    }

    #[must_use]
    pub fn from_f64_integer(value: f64) -> Option<Self> {
        (value.is_finite() && value.fract() == 0.0)
            .then(|| BigInt::from_f64(value).map(|value| Self(Rc::new(value))))
            .flatten()
    }

    #[must_use]
    pub fn parse_digits(digits: &str, radix: u32) -> Option<Self> {
        BigInt::parse_bytes(digits.as_bytes(), radix).map(|value| Self(Rc::new(value)))
    }

    #[must_use]
    pub fn parse_string(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return Some(Self::zero());
        }
        let (digits, radix) = if let Some(digits) =
            text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"))
        {
            (digits, 16)
        } else if let Some(digits) = text.strip_prefix("0o").or_else(|| text.strip_prefix("0O")) {
            (digits, 8)
        } else if let Some(digits) = text.strip_prefix("0b").or_else(|| text.strip_prefix("0B")) {
            (digits, 2)
        } else {
            return text
                .parse::<BigInt>()
                .ok()
                .map(|value| Self(Rc::new(value)));
        };
        Self::parse_digits(digits, radix)
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    #[must_use]
    pub fn is_one(&self) -> bool {
        self.0.as_ref() == &BigInt::from(1_u8)
    }

    #[must_use]
    pub fn is_negative_one(&self) -> bool {
        self.0.as_ref() == &BigInt::from(-1_i8)
    }

    #[must_use]
    pub fn is_odd(&self) -> bool {
        (self.0.as_ref() & BigInt::from(1_u8)) == BigInt::from(1_u8)
    }

    #[must_use]
    pub fn bit_len(&self) -> u64 {
        self.0.bits()
    }

    #[must_use]
    pub fn unchanged_by_as_uint_n(&self, bits: usize) -> bool {
        !self.is_negative()
            && u64::try_from(bits).is_ok_and(|bits| self.is_zero() || self.bit_len() <= bits)
    }

    #[must_use]
    pub fn unchanged_by_as_int_n(&self, bits: usize) -> bool {
        u64::try_from(bits).is_ok_and(|bits| self.is_zero() || self.bit_len() < bits)
    }

    #[must_use]
    pub fn to_f64(&self) -> Option<f64> {
        self.0.to_f64()
    }

    #[must_use]
    pub fn to_u32(&self) -> Option<u32> {
        self.0.to_u32()
    }

    #[must_use]
    pub fn to_u64(&self) -> Option<u64> {
        self.0.to_u64()
    }

    #[must_use]
    pub fn to_i64(&self) -> Option<i64> {
        self.0.to_i64()
    }

    #[must_use]
    pub fn to_usize(&self) -> Option<usize> {
        self.0.to_usize()
    }

    #[must_use]
    pub fn abs(&self) -> Self {
        Self(Rc::new(self.0.abs()))
    }

    #[must_use]
    pub fn negated(&self) -> Self {
        Self(Rc::new(-self.0.as_ref()))
    }

    #[must_use]
    pub fn bitwise_not(&self) -> Self {
        Self(Rc::new(!self.0.as_ref()))
    }

    #[must_use]
    pub fn add(&self, other: &Self) -> Self {
        Self(Rc::new(self.0.as_ref() + other.0.as_ref()))
    }

    #[must_use]
    pub fn sub(&self, other: &Self) -> Self {
        Self(Rc::new(self.0.as_ref() - other.0.as_ref()))
    }

    #[must_use]
    pub fn mul(&self, other: &Self) -> Self {
        Self(Rc::new(self.0.as_ref() * other.0.as_ref()))
    }

    #[must_use]
    pub fn div(&self, other: &Self) -> Option<Self> {
        (!other.is_zero()).then(|| Self(Rc::new(self.0.as_ref() / other.0.as_ref())))
    }

    #[must_use]
    pub fn rem(&self, other: &Self) -> Option<Self> {
        (!other.is_zero()).then(|| Self(Rc::new(self.0.as_ref() % other.0.as_ref())))
    }

    #[must_use]
    pub fn pow(&self, exponent: u32) -> Self {
        Self(Rc::new(self.0.pow(exponent)))
    }

    #[must_use]
    pub fn bitand(&self, other: &Self) -> Self {
        Self(Rc::new(self.0.as_ref() & other.0.as_ref()))
    }

    #[must_use]
    pub fn bitor(&self, other: &Self) -> Self {
        Self(Rc::new(self.0.as_ref() | other.0.as_ref()))
    }

    #[must_use]
    pub fn bitxor(&self, other: &Self) -> Self {
        Self(Rc::new(self.0.as_ref() ^ other.0.as_ref()))
    }

    #[must_use]
    pub fn shift_left(&self, count: usize) -> Self {
        Self(Rc::new(self.0.as_ref() << count))
    }

    #[must_use]
    pub fn shift_right(&self, count: usize) -> Self {
        Self(Rc::new(self.0.as_ref() >> count))
    }

    #[must_use]
    pub fn to_string_radix(&self, radix: u32) -> String {
        self.0.to_str_radix(radix)
    }

    #[must_use]
    pub fn as_uint_n(&self, bits: usize) -> Self {
        if bits == 0 {
            return Self::zero();
        }
        if self.unchanged_by_as_uint_n(bits) {
            return self.clone();
        }
        let modulus = BigInt::from(1_u8) << bits;
        let mut value = self.0.as_ref() % &modulus;
        if value.is_negative() {
            value += &modulus;
        }
        Self(Rc::new(value))
    }

    #[must_use]
    pub fn as_int_n(&self, bits: usize) -> Self {
        if bits == 0 {
            return Self::zero();
        }
        if self.unchanged_by_as_int_n(bits) {
            return self.clone();
        }
        let unsigned = self.as_uint_n(bits);
        let boundary = BigInt::from(1_u8) << bits.saturating_sub(1);
        if unsigned.0.as_ref() < &boundary {
            return unsigned;
        }
        let modulus = BigInt::from(1_u8) << bits;
        Self(Rc::new(unsigned.0.as_ref() - modulus))
    }

    #[must_use]
    pub fn compare_number(&self, number: f64) -> Option<Ordering> {
        if number.is_nan() {
            return None;
        }
        if number == f64::INFINITY {
            return Some(Ordering::Less);
        }
        if number == f64::NEG_INFINITY {
            return Some(Ordering::Greater);
        }
        let truncated = BigInt::from_f64(number.trunc())?;
        let ordering = self.0.as_ref().cmp(&truncated);
        if ordering != Ordering::Equal || number.fract() == 0.0 {
            return Some(ordering);
        }
        Some(if number.is_sign_positive() {
            Ordering::Less
        } else {
            Ordering::Greater
        })
    }

    #[must_use]
    pub fn equals_number(&self, number: f64) -> bool {
        number.is_finite()
            && number.fract() == 0.0
            && BigInt::from_f64(number)
                .as_ref()
                .is_some_and(|integer| integer == self.0.as_ref())
    }
}

impl fmt::Display for JsBigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
