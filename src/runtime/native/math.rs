use std::f64::consts::{E, FRAC_1_SQRT_2, LN_2, LN_10, LOG2_E, LOG10_E, PI, SQRT_2};

use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call_args::RuntimeCallArgs,
    runtime::numeric::number_to_uint32,
    value::{ObjectId, Value},
};

use super::{
    MATH_ABS_NAME, MATH_ACOS_NAME, MATH_ACOSH_NAME, MATH_ASIN_NAME, MATH_ASINH_NAME,
    MATH_ATAN_NAME, MATH_ATAN2_NAME, MATH_ATANH_NAME, MATH_CBRT_NAME, MATH_CEIL_NAME,
    MATH_CLZ32_NAME, MATH_COS_NAME, MATH_COSH_NAME, MATH_EXP_NAME, MATH_EXPM1_NAME,
    MATH_FLOOR_NAME, MATH_FROUND_NAME, MATH_HYPOT_NAME, MATH_IMUL_NAME, MATH_LOG_NAME,
    MATH_LOG1P_NAME, MATH_LOG2_NAME, MATH_LOG10_NAME, MATH_MAX_NAME, MATH_MIN_NAME, MATH_NAME,
    MATH_POW_NAME, MATH_RANDOM_NAME, MATH_ROUND_NAME, MATH_SIGN_NAME, MATH_SIN_NAME,
    MATH_SINH_NAME, MATH_SQRT_NAME, MATH_TAN_NAME, MATH_TANH_NAME, MATH_TRUNC_NAME,
    NativeFunctionKind,
};

const MATH_E_NAME: &str = "E";
const MATH_LN10_NAME: &str = "LN10";
const MATH_LN2_NAME: &str = "LN2";
const MATH_LOG10E_NAME: &str = "LOG10E";
const MATH_LOG2E_NAME: &str = "LOG2E";
const MATH_PI_NAME: &str = "PI";
const MATH_SQRT1_2_NAME: &str = "SQRT1_2";
const MATH_SQRT2_NAME: &str = "SQRT2";
const RANDOM_DENOMINATOR: f64 = 9_007_199_254_740_992.0;
const RANDOM_FRACTION_BITS: u32 = 53;
const RANDOM_HIGH_SCALE: f64 = 2_097_152.0;
const RANDOM_LOW_BITS: u32 = 21;
const RANDOM_LOW_MASK: u64 = (1_u64 << RANDOM_LOW_BITS) - 1;
const RANDOM_XOR_SHIFT_A: u32 = 13;
const RANDOM_XOR_SHIFT_B: u32 = 7;
const RANDOM_XOR_SHIFT_C: u32 = 17;

impl Context {
    pub(super) fn math_object_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(MATH_NAME) {
            return Ok(binding.value());
        }

        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.define_math_constant(object, MATH_E_NAME, E)?;
        self.define_math_constant(object, MATH_LN10_NAME, LN_10)?;
        self.define_math_constant(object, MATH_LN2_NAME, LN_2)?;
        self.define_math_constant(object, MATH_LOG10E_NAME, LOG10_E)?;
        self.define_math_constant(object, MATH_LOG2E_NAME, LOG2_E)?;
        self.define_math_constant(object, MATH_PI_NAME, PI)?;
        self.define_math_constant(object, MATH_SQRT1_2_NAME, FRAC_1_SQRT_2)?;
        self.define_math_constant(object, MATH_SQRT2_NAME, SQRT_2)?;

        self.define_math_method(object, MATH_ABS_NAME, NativeFunctionKind::MathAbs)?;
        self.define_math_method(object, MATH_ACOS_NAME, NativeFunctionKind::MathAcos)?;
        self.define_math_method(object, MATH_ACOSH_NAME, NativeFunctionKind::MathAcosh)?;
        self.define_math_method(object, MATH_ASIN_NAME, NativeFunctionKind::MathAsin)?;
        self.define_math_method(object, MATH_ASINH_NAME, NativeFunctionKind::MathAsinh)?;
        self.define_math_method(object, MATH_ATAN_NAME, NativeFunctionKind::MathAtan)?;
        self.define_math_method(object, MATH_ATAN2_NAME, NativeFunctionKind::MathAtan2)?;
        self.define_math_method(object, MATH_ATANH_NAME, NativeFunctionKind::MathAtanh)?;
        self.define_math_method(object, MATH_CBRT_NAME, NativeFunctionKind::MathCbrt)?;
        self.define_math_method(object, MATH_CEIL_NAME, NativeFunctionKind::MathCeil)?;
        self.define_math_method(object, MATH_CLZ32_NAME, NativeFunctionKind::MathClz32)?;
        self.define_math_method(object, MATH_COS_NAME, NativeFunctionKind::MathCos)?;
        self.define_math_method(object, MATH_COSH_NAME, NativeFunctionKind::MathCosh)?;
        self.define_math_method(object, MATH_EXP_NAME, NativeFunctionKind::MathExp)?;
        self.define_math_method(object, MATH_EXPM1_NAME, NativeFunctionKind::MathExpm1)?;
        self.define_math_method(object, MATH_FLOOR_NAME, NativeFunctionKind::MathFloor)?;
        self.define_math_method(object, MATH_FROUND_NAME, NativeFunctionKind::MathFround)?;
        self.define_math_method(object, MATH_HYPOT_NAME, NativeFunctionKind::MathHypot)?;
        self.define_math_method(object, MATH_IMUL_NAME, NativeFunctionKind::MathImul)?;
        self.define_math_method(object, MATH_LOG_NAME, NativeFunctionKind::MathLog)?;
        self.define_math_method(object, MATH_LOG10_NAME, NativeFunctionKind::MathLog10)?;
        self.define_math_method(object, MATH_LOG1P_NAME, NativeFunctionKind::MathLog1p)?;
        self.define_math_method(object, MATH_LOG2_NAME, NativeFunctionKind::MathLog2)?;
        self.define_math_method(object, MATH_MAX_NAME, NativeFunctionKind::MathMax)?;
        self.define_math_method(object, MATH_MIN_NAME, NativeFunctionKind::MathMin)?;
        self.define_math_method(object, MATH_POW_NAME, NativeFunctionKind::MathPow)?;
        self.define_math_method(object, MATH_RANDOM_NAME, NativeFunctionKind::MathRandom)?;
        self.define_math_method(object, MATH_ROUND_NAME, NativeFunctionKind::MathRound)?;
        self.define_math_method(object, MATH_SIGN_NAME, NativeFunctionKind::MathSign)?;
        self.define_math_method(object, MATH_SIN_NAME, NativeFunctionKind::MathSin)?;
        self.define_math_method(object, MATH_SINH_NAME, NativeFunctionKind::MathSinh)?;
        self.define_math_method(object, MATH_SQRT_NAME, NativeFunctionKind::MathSqrt)?;
        self.define_math_method(object, MATH_TAN_NAME, NativeFunctionKind::MathTan)?;
        self.define_math_method(object, MATH_TANH_NAME, NativeFunctionKind::MathTanh)?;
        self.define_math_method(object, MATH_TRUNC_NAME, NativeFunctionKind::MathTrunc)?;

        let value = Value::Object(object);
        self.insert_global_builtin(MATH_NAME, value.clone())?;
        Ok(value)
    }

    pub(super) fn eval_math_abs(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::abs)
    }

    pub(super) fn eval_math_acos(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::acos)
    }

    pub(super) fn eval_math_acosh(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::acosh)
    }

    pub(super) fn eval_math_asin(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::asin)
    }

    pub(super) fn eval_math_asinh(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::asinh)
    }

    pub(super) fn eval_math_atan(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::atan)
    }

    pub(super) fn eval_math_atan2(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let (y, x) = Self::eval_math_binary_values(args);
        let y = Self::math_arg_or_nan(y.as_ref());
        let x = x.as_ref().map_or(f64::NAN, Self::value_to_number);
        self.checked_value(Value::Number(y.atan2(x)))
    }

    pub(super) fn eval_math_atanh(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::atanh)
    }

    pub(super) fn eval_math_cbrt(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::cbrt)
    }

    pub(super) fn eval_math_ceil(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::ceil)
    }

    pub(super) fn eval_math_clz32(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let value = Self::eval_math_unary_value(args);
        let unsigned = number_to_uint32(Self::math_arg_or_nan(value.as_ref()), MATH_CLZ32_NAME)?;
        self.checked_value(Value::Number(f64::from(unsigned.leading_zeros())))
    }

    pub(super) fn eval_math_cos(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::cos)
    }

    pub(super) fn eval_math_cosh(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::cosh)
    }

    pub(super) fn eval_math_exp(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::exp)
    }

    pub(super) fn eval_math_expm1(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::exp_m1)
    }

    pub(super) fn eval_math_floor(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::floor)
    }

    pub(super) fn eval_math_fround(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let value = Self::eval_math_unary_value(args);
        self.checked_value(Value::Number(Self::fround_to_number(
            Self::math_arg_or_nan(value.as_ref()),
        )))
    }

    pub(super) fn eval_math_hypot(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = Self::eval_math_args(args);
        let mut has_infinity = false;
        let mut has_nan = false;
        let mut magnitude = 0.0_f64;
        for value in values {
            let value = Self::value_to_number(&value);
            if value.is_infinite() {
                has_infinity = true;
            } else if value.is_nan() {
                has_nan = true;
            } else {
                magnitude = magnitude.hypot(value);
            }
        }
        if has_infinity {
            return self.checked_value(Value::Number(f64::INFINITY));
        }
        if has_nan {
            return self.checked_value(Value::Number(f64::NAN));
        }
        self.checked_value(Value::Number(magnitude))
    }

    pub(super) fn eval_math_imul(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let (left, right) = Self::eval_math_binary_values(args);
        let left = number_to_uint32(Self::math_arg_or_nan(left.as_ref()), MATH_IMUL_NAME)?;
        let right = number_to_uint32(
            right.as_ref().map_or(f64::NAN, Self::value_to_number),
            MATH_IMUL_NAME,
        )?;
        let product = left.wrapping_mul(right);
        self.checked_value(Value::Number(f64::from(i32::from_ne_bytes(
            product.to_ne_bytes(),
        ))))
    }

    pub(super) fn eval_math_log(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::ln)
    }

    pub(super) fn eval_math_log10(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::log10)
    }

    pub(super) fn eval_math_log1p(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::ln_1p)
    }

    pub(super) fn eval_math_log2(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::log2)
    }

    pub(super) fn eval_math_max(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = Self::eval_math_args(args);
        let mut maximum = f64::NEG_INFINITY;
        for value in values {
            let value = Self::value_to_number(&value);
            if value.is_nan() {
                return self.checked_value(Value::Number(f64::NAN));
            }
            if value > maximum || Self::should_replace_max_zero(maximum, value) {
                maximum = value;
            }
        }
        self.checked_value(Value::Number(maximum))
    }

    pub(super) fn eval_math_min(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let values = Self::eval_math_args(args);
        let mut minimum = f64::INFINITY;
        for value in values {
            let value = Self::value_to_number(&value);
            if value.is_nan() {
                return self.checked_value(Value::Number(f64::NAN));
            }
            if value < minimum || Self::should_replace_min_zero(minimum, value) {
                minimum = value;
            }
        }
        self.checked_value(Value::Number(minimum))
    }

    pub(super) fn eval_math_pow(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let (base, exponent) = Self::eval_math_binary_values(args);
        let base = Self::math_arg_or_nan(base.as_ref());
        let exponent = exponent.as_ref().map_or(f64::NAN, Self::value_to_number);
        self.checked_value(Value::Number(base.powf(exponent)))
    }

    pub(super) fn eval_math_random(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        Self::eval_math_discard_args(args);
        Ok(Value::Number(self.next_math_random()?))
    }

    pub(super) fn eval_math_round(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let value = Self::eval_math_unary_value(args);
        self.checked_value(Value::Number(Self::round_to_nearest_toward_positive(
            Self::math_arg_or_nan(value.as_ref()),
        )))
    }

    pub(super) fn eval_math_sign(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let value = Self::eval_math_unary_value(args);
        let value = Self::math_arg_or_nan(value.as_ref());
        if value.is_nan() || value == 0.0 {
            return self.checked_value(Value::Number(value));
        }
        if value.is_sign_negative() {
            return self.checked_value(Value::Number(-1.0));
        }
        self.checked_value(Value::Number(1.0))
    }

    pub(super) fn eval_math_sin(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::sin)
    }

    pub(super) fn eval_math_sinh(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::sinh)
    }

    pub(super) fn eval_math_sqrt(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::sqrt)
    }

    pub(super) fn eval_math_tan(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::tan)
    }

    pub(super) fn eval_math_tanh(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::tanh)
    }

    pub(super) fn eval_math_trunc(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        self.eval_math_unary(args, f64::trunc)
    }

    fn define_math_constant(&mut self, object: ObjectId, name: &str, value: f64) -> Result<()> {
        self.define_non_enumerable_object_property(object, name, Value::Number(value))
    }

    fn define_math_method(
        &mut self,
        object: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined)?;
        self.define_non_enumerable_object_property(object, name, function)
    }

    fn eval_math_args(args: RuntimeCallArgs<'_>) -> Vec<Value> {
        args.evaluate()
    }

    fn eval_math_unary(
        &self,
        args: RuntimeCallArgs<'_>,
        operation: fn(f64) -> f64,
    ) -> Result<Value> {
        let value = Self::eval_math_unary_value(args);
        self.checked_value(Value::Number(operation(Self::math_arg_or_nan(
            value.as_ref(),
        ))))
    }

    fn eval_math_unary_value(args: RuntimeCallArgs<'_>) -> Option<Value> {
        args.unary_value()
    }

    fn eval_math_binary_values(args: RuntimeCallArgs<'_>) -> (Option<Value>, Option<Value>) {
        args.binary_values()
    }

    fn eval_math_discard_args(args: RuntimeCallArgs<'_>) {
        args.discard();
    }

    fn math_arg_or_nan(value: Option<&Value>) -> f64 {
        value.map_or(f64::NAN, Self::value_to_number)
    }

    fn fround_to_number(value: f64) -> f64 {
        f64::from(Self::round_to_binary32(value))
    }

    fn next_math_random(&mut self) -> Result<f64> {
        let mut state = self.random_state;
        state ^= state << RANDOM_XOR_SHIFT_A;
        state ^= state >> RANDOM_XOR_SHIFT_B;
        state ^= state << RANDOM_XOR_SHIFT_C;
        self.random_state = state;

        let fraction_bits = state >> (u64::BITS - RANDOM_FRACTION_BITS);
        let high = u32::try_from(fraction_bits >> RANDOM_LOW_BITS)
            .map_err(|_| Error::runtime("Math.random high bits conversion overflowed"))?;
        let low = u32::try_from(fraction_bits & RANDOM_LOW_MASK)
            .map_err(|_| Error::runtime("Math.random low bits conversion overflowed"))?;
        Ok(f64::from(high).mul_add(RANDOM_HIGH_SCALE, f64::from(low)) / RANDOM_DENOMINATOR)
    }

    #[allow(clippy::cast_possible_truncation)]
    const fn round_to_binary32(value: f64) -> f32 {
        // This cast is the Rust standard-library path for ECMAScript binary32 rounding.
        value as f32
    }

    fn round_to_nearest_toward_positive(value: f64) -> f64 {
        if value.is_nan() || value.is_infinite() || value == 0.0 {
            return value;
        }
        let rounded = (value + 0.5).floor();
        if rounded == 0.0 && value.is_sign_negative() {
            return -0.0;
        }
        rounded
    }

    fn should_replace_max_zero(current: f64, candidate: f64) -> bool {
        current == 0.0
            && candidate == 0.0
            && current.is_sign_negative()
            && candidate.is_sign_positive()
    }

    fn should_replace_min_zero(current: f64, candidate: f64) -> bool {
        current == 0.0
            && candidate == 0.0
            && current.is_sign_positive()
            && candidate.is_sign_negative()
    }
}
