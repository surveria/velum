use std::f64::consts::{E, FRAC_1_SQRT_2, LN_2, LN_10, LOG2_E, LOG10_E, PI, SQRT_2};

use crate::{
    ast::Expr,
    error::Result,
    runtime::Context,
    runtime_numeric::number_to_uint32,
    value::{ObjectId, Value},
};

use super::{
    MATH_ABS_NAME, MATH_ACOS_NAME, MATH_ACOSH_NAME, MATH_ASIN_NAME, MATH_ASINH_NAME,
    MATH_ATAN_NAME, MATH_ATAN2_NAME, MATH_ATANH_NAME, MATH_CBRT_NAME, MATH_CEIL_NAME,
    MATH_CLZ32_NAME, MATH_COS_NAME, MATH_COSH_NAME, MATH_EXP_NAME, MATH_EXPM1_NAME,
    MATH_FLOOR_NAME, MATH_FROUND_NAME, MATH_HYPOT_NAME, MATH_IMUL_NAME, MATH_LOG_NAME,
    MATH_LOG1P_NAME, MATH_LOG2_NAME, MATH_LOG10_NAME, MATH_MAX_NAME, MATH_MIN_NAME, MATH_NAME,
    MATH_POW_NAME, MATH_ROUND_NAME, MATH_SIGN_NAME, MATH_SIN_NAME, MATH_SINH_NAME, MATH_SQRT_NAME,
    MATH_TAN_NAME, MATH_TANH_NAME, MATH_TRUNC_NAME, NativeFunctionKind,
};

const MATH_E_NAME: &str = "E";
const MATH_LN10_NAME: &str = "LN10";
const MATH_LN2_NAME: &str = "LN2";
const MATH_LOG10E_NAME: &str = "LOG10E";
const MATH_LOG2E_NAME: &str = "LOG2E";
const MATH_PI_NAME: &str = "PI";
const MATH_SQRT1_2_NAME: &str = "SQRT1_2";
const MATH_SQRT2_NAME: &str = "SQRT2";

impl Context {
    pub(super) fn math_object_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(MATH_NAME) {
            return Ok(binding.value());
        }

        let object = self.objects.create_with_prototype_id(
            None,
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

    pub(super) fn eval_math_abs(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::abs)
    }

    pub(super) fn eval_math_acos(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::acos)
    }

    pub(super) fn eval_math_acosh(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::acosh)
    }

    pub(super) fn eval_math_asin(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::asin)
    }

    pub(super) fn eval_math_asinh(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::asinh)
    }

    pub(super) fn eval_math_atan(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::atan)
    }

    pub(super) fn eval_math_atan2(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        let y = Self::math_arg_or_nan(values.first());
        let x = values.get(1).map_or(f64::NAN, Self::value_to_number);
        Ok(Value::Number(y.atan2(x)))
    }

    pub(super) fn eval_math_atanh(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::atanh)
    }

    pub(super) fn eval_math_cbrt(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::cbrt)
    }

    pub(super) fn eval_math_ceil(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::ceil)
    }

    pub(super) fn eval_math_clz32(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        let unsigned = number_to_uint32(Self::math_arg_or_nan(values.first()), MATH_CLZ32_NAME)?;
        Ok(Value::Number(f64::from(unsigned.leading_zeros())))
    }

    pub(super) fn eval_math_cos(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::cos)
    }

    pub(super) fn eval_math_cosh(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::cosh)
    }

    pub(super) fn eval_math_exp(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::exp)
    }

    pub(super) fn eval_math_expm1(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::exp_m1)
    }

    pub(super) fn eval_math_floor(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::floor)
    }

    pub(super) fn eval_math_fround(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        Ok(Value::Number(Self::fround_to_number(
            Self::math_arg_or_nan(values.first()),
        )))
    }

    pub(super) fn eval_math_hypot(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
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
            return Ok(Value::Number(f64::INFINITY));
        }
        if has_nan {
            return Ok(Value::Number(f64::NAN));
        }
        Ok(Value::Number(magnitude))
    }

    pub(super) fn eval_math_imul(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        let left = number_to_uint32(Self::math_arg_or_nan(values.first()), MATH_IMUL_NAME)?;
        let right = number_to_uint32(
            values.get(1).map_or(f64::NAN, Self::value_to_number),
            MATH_IMUL_NAME,
        )?;
        let product = left.wrapping_mul(right);
        Ok(Value::Number(f64::from(i32::from_ne_bytes(
            product.to_ne_bytes(),
        ))))
    }

    pub(super) fn eval_math_log(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::ln)
    }

    pub(super) fn eval_math_log10(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::log10)
    }

    pub(super) fn eval_math_log1p(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::ln_1p)
    }

    pub(super) fn eval_math_log2(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::log2)
    }

    pub(super) fn eval_math_max(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        let mut maximum = f64::NEG_INFINITY;
        for value in values {
            let value = Self::value_to_number(&value);
            if value.is_nan() {
                return Ok(Value::Number(f64::NAN));
            }
            if value > maximum || Self::should_replace_max_zero(maximum, value) {
                maximum = value;
            }
        }
        Ok(Value::Number(maximum))
    }

    pub(super) fn eval_math_min(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        let mut minimum = f64::INFINITY;
        for value in values {
            let value = Self::value_to_number(&value);
            if value.is_nan() {
                return Ok(Value::Number(f64::NAN));
            }
            if value < minimum || Self::should_replace_min_zero(minimum, value) {
                minimum = value;
            }
        }
        Ok(Value::Number(minimum))
    }

    pub(super) fn eval_math_pow(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        let base = Self::math_arg_or_nan(values.first());
        let exponent = values.get(1).map_or(f64::NAN, Self::value_to_number);
        Ok(Value::Number(base.powf(exponent)))
    }

    pub(super) fn eval_math_round(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        Ok(Value::Number(Self::round_to_nearest_toward_positive(
            Self::math_arg_or_nan(values.first()),
        )))
    }

    pub(super) fn eval_math_sign(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        let value = Self::math_arg_or_nan(values.first());
        if value.is_nan() || value == 0.0 {
            return Ok(Value::Number(value));
        }
        if value.is_sign_negative() {
            return Ok(Value::Number(-1.0));
        }
        Ok(Value::Number(1.0))
    }

    pub(super) fn eval_math_sin(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::sin)
    }

    pub(super) fn eval_math_sinh(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::sinh)
    }

    pub(super) fn eval_math_sqrt(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::sqrt)
    }

    pub(super) fn eval_math_tan(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::tan)
    }

    pub(super) fn eval_math_tanh(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::tanh)
    }

    pub(super) fn eval_math_trunc(&mut self, args: &[Expr]) -> Result<Value> {
        self.eval_math_unary(args, f64::trunc)
    }

    fn define_math_constant(&mut self, object: ObjectId, name: &str, value: f64) -> Result<()> {
        self.objects.define_non_enumerable(
            object,
            name.to_owned(),
            Value::Number(value),
            self.limits.max_object_properties,
        )
    }

    fn define_math_method(
        &mut self,
        object: ObjectId,
        name: &str,
        kind: NativeFunctionKind,
    ) -> Result<()> {
        let function = self.create_native_function(kind, Value::Undefined);
        self.objects.define_non_enumerable(
            object,
            name.to_owned(),
            function,
            self.limits.max_object_properties,
        )
    }

    fn eval_math_args(&mut self, args: &[Expr]) -> Result<Vec<Value>> {
        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval_expr(arg)?);
        }
        Ok(values)
    }

    fn eval_math_unary(&mut self, args: &[Expr], operation: fn(f64) -> f64) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        Ok(Value::Number(operation(Self::math_arg_or_nan(
            values.first(),
        ))))
    }

    fn math_arg_or_nan(value: Option<&Value>) -> f64 {
        value.map_or(f64::NAN, Self::value_to_number)
    }

    fn fround_to_number(value: f64) -> f64 {
        f64::from(Self::round_to_binary32(value))
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
