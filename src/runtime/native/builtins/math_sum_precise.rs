use core::cmp::Ordering;

use crate::{
    error::{Error, Result},
    value::Value,
};

const SUM_EXPONENT_BIAS: i32 = 1023;
const SUM_FRACTION_BITS_I32: i32 = 52;
const SUM_FRACTION_BITS_U32: u32 = 52;
const SUM_FRACTION_MASK: u64 = (1_u64 << SUM_FRACTION_BITS_U32) - 1;
const SUM_HIDDEN_BIT: u64 = 1_u64 << SUM_FRACTION_BITS_U32;
const SUM_EXPONENT_MASK: u64 = 0x7ff;
const SUM_SIGN_BIT: u64 = 1_u64 << 63;
const SUM_MIN_EXPONENT: i32 = -1074;
const SUM_MAX_EXPONENT: i32 = 1023;
const SUM_LIMB_BITS: usize = 64;
const SUM_LIMB_COUNT: usize = 33;
const SUM_RETAINED_BITS: usize = 53;

const FLAG_POSITIVE_INFINITY: u8 = 1;
const FLAG_NEGATIVE_INFINITY: u8 = 1 << 1;
const FLAG_NAN: u8 = 1 << 2;
const FLAG_NON_ZERO_FINITE: u8 = 1 << 3;
const FLAG_POSITIVE_ZERO: u8 = 1 << 4;

#[derive(Debug, Clone)]
pub(super) struct PreciseFiniteSum {
    positive: FixedBigUint,
    negative: FixedBigUint,
    flags: PreciseSumFlags,
}

#[derive(Debug, Clone, Copy, Default)]
struct PreciseSumFlags {
    bits: u8,
}

#[derive(Debug, Clone)]
struct FixedBigUint {
    limbs: Vec<u64>,
}

impl PreciseFiniteSum {
    pub(super) fn new() -> Self {
        Self {
            positive: FixedBigUint::new(),
            negative: FixedBigUint::new(),
            flags: PreciseSumFlags::new(),
        }
    }

    pub(super) fn add_value(&mut self, value: &Value) -> Result<()> {
        let Some(number) = value.as_number() else {
            return Err(Error::type_error(
                "Math.sumPrecise only accepts Number values",
            ));
        };
        self.add_number(number)
    }

    pub(super) fn finish(&self) -> Result<f64> {
        if self.flags.saw_nan() || self.flags.saw_both_infinities() {
            return Ok(f64::NAN);
        }
        if self.flags.saw_positive_infinity() {
            return Ok(f64::INFINITY);
        }
        if self.flags.saw_negative_infinity() {
            return Ok(f64::NEG_INFINITY);
        }

        match self.positive.cmp_magnitude(&self.negative) {
            Ordering::Greater => self.positive.subtract(&self.negative)?.to_f64(false),
            Ordering::Less => self.negative.subtract(&self.positive)?.to_f64(true),
            Ordering::Equal => {
                if self.flags.saw_non_zero_finite() || self.flags.saw_positive_zero() {
                    Ok(0.0)
                } else {
                    Ok(-0.0)
                }
            }
        }
    }

    fn add_number(&mut self, value: f64) -> Result<()> {
        if value.is_nan() {
            self.flags.mark_nan();
            return Ok(());
        }
        if value == f64::INFINITY {
            self.flags.mark_positive_infinity();
            return Ok(());
        }
        if value == f64::NEG_INFINITY {
            self.flags.mark_negative_infinity();
            return Ok(());
        }
        if value == 0.0 {
            if value.is_sign_positive() {
                self.flags.mark_positive_zero();
            }
            return Ok(());
        }

        self.flags.mark_non_zero_finite();
        if value.is_sign_negative() {
            self.negative.add_f64_magnitude(value.abs())
        } else {
            self.positive.add_f64_magnitude(value)
        }
    }
}

impl PreciseSumFlags {
    const fn new() -> Self {
        Self { bits: 0 }
    }

    const fn mark_positive_infinity(&mut self) {
        self.bits |= FLAG_POSITIVE_INFINITY;
    }

    const fn mark_negative_infinity(&mut self) {
        self.bits |= FLAG_NEGATIVE_INFINITY;
    }

    const fn mark_nan(&mut self) {
        self.bits |= FLAG_NAN;
    }

    const fn mark_non_zero_finite(&mut self) {
        self.bits |= FLAG_NON_ZERO_FINITE;
    }

    const fn mark_positive_zero(&mut self) {
        self.bits |= FLAG_POSITIVE_ZERO;
    }

    const fn saw_positive_infinity(self) -> bool {
        self.bits & FLAG_POSITIVE_INFINITY != 0
    }

    const fn saw_negative_infinity(self) -> bool {
        self.bits & FLAG_NEGATIVE_INFINITY != 0
    }

    const fn saw_both_infinities(self) -> bool {
        self.saw_positive_infinity() && self.saw_negative_infinity()
    }

    const fn saw_nan(self) -> bool {
        self.bits & FLAG_NAN != 0
    }

    const fn saw_non_zero_finite(self) -> bool {
        self.bits & FLAG_NON_ZERO_FINITE != 0
    }

    const fn saw_positive_zero(self) -> bool {
        self.bits & FLAG_POSITIVE_ZERO != 0
    }
}

impl FixedBigUint {
    fn new() -> Self {
        Self {
            limbs: vec![0; SUM_LIMB_COUNT],
        }
    }

    fn add_f64_magnitude(&mut self, value: f64) -> Result<()> {
        let bits = value.to_bits();
        let exponent_bits = (bits >> SUM_FRACTION_BITS_U32) & SUM_EXPONENT_MASK;
        let fraction = bits & SUM_FRACTION_MASK;
        let (mantissa, shift) = if exponent_bits == 0 {
            (fraction, 0)
        } else {
            let exponent = i32::try_from(exponent_bits)
                .map_err(|_| Error::runtime("f64 exponent conversion overflowed"))?
                - SUM_EXPONENT_BIAS;
            let shift = exponent
                .checked_sub(SUM_FRACTION_BITS_I32)
                .and_then(|value| value.checked_sub(SUM_MIN_EXPONENT))
                .ok_or_else(|| Error::runtime("f64 significand shift overflowed"))?;
            (
                SUM_HIDDEN_BIT | fraction,
                usize::try_from(shift)
                    .map_err(|_| Error::runtime("f64 significand shift was negative"))?,
            )
        };

        if mantissa == 0 {
            return Ok(());
        }
        self.add_shifted(mantissa, shift)
    }

    fn add_shifted(&mut self, mantissa: u64, shift: usize) -> Result<()> {
        let limb_index = shift / SUM_LIMB_BITS;
        let bit_shift = shift % SUM_LIMB_BITS;
        if bit_shift == 0 {
            return self.add_limb(limb_index, mantissa);
        }

        self.add_limb(limb_index, mantissa << bit_shift)?;
        let high_shift = SUM_LIMB_BITS
            .checked_sub(bit_shift)
            .ok_or_else(|| Error::runtime("f64 significand high shift underflowed"))?;
        let high = mantissa >> high_shift;
        if high == 0 {
            return Ok(());
        }
        self.add_limb(
            limb_index
                .checked_add(1)
                .ok_or_else(|| Error::runtime("f64 significand limb index overflowed"))?,
            high,
        )
    }

    fn add_limb(&mut self, index: usize, value: u64) -> Result<()> {
        let mut index = index;
        let mut addend = value;
        while addend != 0 {
            let limb = self
                .limbs
                .get_mut(index)
                .ok_or_else(|| Error::runtime("precise sum accumulator overflowed"))?;
            let (sum, overflowed) = limb.overflowing_add(addend);
            *limb = sum;
            if !overflowed {
                return Ok(());
            }
            addend = 1;
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::runtime("precise sum carry index overflowed"))?;
        }
        Ok(())
    }

    fn cmp_magnitude(&self, other: &Self) -> Ordering {
        for (left, right) in self.limbs.iter().rev().zip(other.limbs.iter().rev()) {
            match left.cmp(right) {
                Ordering::Equal => {}
                ordering => return ordering,
            }
        }
        Ordering::Equal
    }

    fn subtract(&self, other: &Self) -> Result<Self> {
        let mut result = Self::new();
        let mut borrow = false;
        for ((out, left), right) in result
            .limbs
            .iter_mut()
            .zip(self.limbs.iter())
            .zip(other.limbs.iter())
        {
            let lhs = u128::from(*left);
            let rhs = u128::from(*right) + u128::from(u8::from(borrow));
            if lhs >= rhs {
                *out = u64::try_from(lhs - rhs)
                    .map_err(|_| Error::runtime("precise sum subtraction overflowed"))?;
                borrow = false;
            } else {
                *out = u64::try_from((1_u128 << SUM_LIMB_BITS) + lhs - rhs)
                    .map_err(|_| Error::runtime("precise sum subtraction overflowed"))?;
                borrow = true;
            }
        }
        if borrow {
            return Err(Error::runtime("precise sum subtraction underflowed"));
        }
        Ok(result)
    }

    fn to_f64(&self, negative: bool) -> Result<f64> {
        let Some(highest_bit) = self.highest_bit()? else {
            return Ok(if negative { -0.0 } else { 0.0 });
        };
        let exponent = i32::try_from(highest_bit)
            .map_err(|_| Error::runtime("precise sum exponent conversion overflowed"))?
            .checked_add(SUM_MIN_EXPONENT)
            .ok_or_else(|| Error::runtime("precise sum exponent overflowed"))?;
        if exponent > SUM_MAX_EXPONENT {
            return Ok(if negative {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            });
        }
        if exponent < -1022 {
            let mantissa = self.low_u64_after_shift(0)?;
            return Ok(f64::from_bits(Self::sign_bits(negative) | mantissa));
        }

        let shift = highest_bit.saturating_sub(SUM_RETAINED_BITS - 1);
        let mut retained = self.low_u64_after_shift(shift)?;
        let mut rounded_exponent = exponent;
        if shift > 0 && self.should_round_up(retained, shift) {
            retained = retained
                .checked_add(1)
                .ok_or_else(|| Error::runtime("precise sum retained bits overflowed"))?;
            if retained == (1_u64 << SUM_RETAINED_BITS) {
                retained >>= 1;
                rounded_exponent = rounded_exponent
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("precise sum rounded exponent overflowed"))?;
            }
        }
        if rounded_exponent > SUM_MAX_EXPONENT {
            return Ok(if negative {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            });
        }

        let exponent_bits = u64::try_from(
            rounded_exponent
                .checked_add(SUM_EXPONENT_BIAS)
                .ok_or_else(|| Error::runtime("precise sum biased exponent overflowed"))?,
        )
        .map_err(|_| Error::runtime("precise sum biased exponent was negative"))?;
        let fraction = retained & SUM_FRACTION_MASK;
        Ok(f64::from_bits(
            Self::sign_bits(negative) | (exponent_bits << SUM_FRACTION_BITS_U32) | fraction,
        ))
    }

    fn highest_bit(&self) -> Result<Option<usize>> {
        for (limb_index, limb) in self.limbs.iter().enumerate().rev() {
            if *limb == 0 {
                continue;
            }
            let bit_in_limb = usize::try_from(u64::BITS - 1 - limb.leading_zeros())
                .map_err(|_| Error::runtime("precise sum bit index conversion overflowed"))?;
            return limb_index
                .checked_mul(SUM_LIMB_BITS)
                .and_then(|base| base.checked_add(bit_in_limb))
                .map(Some)
                .ok_or_else(|| Error::runtime("precise sum highest bit overflowed"));
        }
        Ok(None)
    }

    fn low_u64_after_shift(&self, shift: usize) -> Result<u64> {
        let limb_index = shift / SUM_LIMB_BITS;
        let bit_shift = shift % SUM_LIMB_BITS;
        let low = self.limbs.get(limb_index).copied().unwrap_or(0) >> bit_shift;
        if bit_shift == 0 {
            return Ok(low);
        }
        let high_shift = SUM_LIMB_BITS
            .checked_sub(bit_shift)
            .ok_or_else(|| Error::runtime("precise sum high shift underflowed"))?;
        let high = self
            .limbs
            .get(limb_index.saturating_add(1))
            .copied()
            .unwrap_or(0)
            << high_shift;
        Ok(low | high)
    }

    fn should_round_up(&self, retained: u64, shift: usize) -> bool {
        let halfway_bit = shift.saturating_sub(1);
        self.bit_at(halfway_bit) && (self.any_bits_below(halfway_bit) || retained % 2 == 1)
    }

    fn bit_at(&self, bit: usize) -> bool {
        let limb_index = bit / SUM_LIMB_BITS;
        let bit_index = bit % SUM_LIMB_BITS;
        self.limbs
            .get(limb_index)
            .is_some_and(|limb| (*limb & (1_u64 << bit_index)) != 0)
    }

    fn any_bits_below(&self, bit_count: usize) -> bool {
        let full_limbs = bit_count / SUM_LIMB_BITS;
        if self.limbs.iter().take(full_limbs).any(|limb| *limb != 0) {
            return true;
        }
        let remaining_bits = bit_count % SUM_LIMB_BITS;
        if remaining_bits == 0 {
            return false;
        }
        let mask = (1_u64 << remaining_bits) - 1;
        self.limbs
            .get(full_limbs)
            .is_some_and(|limb| (*limb & mask) != 0)
    }

    const fn sign_bits(negative: bool) -> u64 {
        if negative { SUM_SIGN_BIT } else { 0 }
    }
}
