use std::f64::consts::{E, FRAC_1_SQRT_2, LN_2, LN_10, LOG2_E, LOG10_E, PI, SQRT_2};

use crate::{
    api::native_call::NativeCallTarget,
    error::{Error, Result},
    runtime::call::RuntimeCallArgs,
    runtime::numeric::{number_exponentiate, number_to_uint32},
    runtime::object::{
        DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey, PropertyUpdate,
        PropertyWritable,
    },
    runtime::{Context, abstract_operations::IteratorStep},
    value::{ObjectId, Value},
};

use super::{
    MATH_ABS_NAME, MATH_ACOS_NAME, MATH_ACOSH_NAME, MATH_ASIN_NAME, MATH_ASINH_NAME,
    MATH_ATAN_NAME, MATH_ATAN2_NAME, MATH_ATANH_NAME, MATH_CBRT_NAME, MATH_CEIL_NAME,
    MATH_CLZ32_NAME, MATH_COS_NAME, MATH_COSH_NAME, MATH_EXP_NAME, MATH_EXPM1_NAME,
    MATH_F16ROUND_NAME, MATH_FLOOR_NAME, MATH_FROUND_NAME, MATH_HYPOT_NAME, MATH_IMUL_NAME,
    MATH_LOG_NAME, MATH_LOG1P_NAME, MATH_LOG2_NAME, MATH_LOG10_NAME, MATH_MAX_NAME, MATH_MIN_NAME,
    MATH_NAME, MATH_POW_NAME, MATH_RANDOM_NAME, MATH_ROUND_NAME, MATH_SIGN_NAME, MATH_SIN_NAME,
    MATH_SINH_NAME, MATH_SQRT_NAME, MATH_SUM_PRECISE_NAME, MATH_TAN_NAME, MATH_TANH_NAME,
    MATH_TRUNC_NAME, NativeFunctionKind,
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
const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const F16_MIN_NORMAL: f64 = 0.000_061_035_156_25;
const F16_MIN_SUBNORMAL: f64 = 0.000_000_059_604_644_775_390_63;
const F16_HALF_MIN_SUBNORMAL: f64 = 0.000_000_029_802_322_387_695_313;
const F16_MAX_FINITE: f64 = 65_504.0;
const F16_OVERFLOW_CUTOFF: f64 = 65_520.0;
const F16_SIGNIFICAND_BITS: i32 = 10;
const ROUND_INTEGER_CUTOFF: f64 = 4_503_599_627_370_496.0;

use super::math_sum_precise::PreciseFiniteSum;

macro_rules! math_unary_method {
    ($runtime_name:ident, $direct_name:ident, $operation:path) => {
        pub(in crate::runtime::native) fn $runtime_name(
            &mut self,
            args: RuntimeCallArgs<'_>,
        ) -> Result<Value> {
            self.$direct_name(args.as_slice())
        }

        pub(in crate::runtime::native) fn $direct_name(&mut self, args: &[Value]) -> Result<Value> {
            self.eval_math_unary_value(args.first(), $operation)
        }
    };
}

fn accurate_atanh(value: f64) -> f64 {
    if value == 0.0 || value.is_nan() {
        return value;
    }
    let magnitude = value.abs();
    let result = if magnitude < 0.5 {
        let square = magnitude * magnitude;
        0.5 * 2.0_f64
            .mul_add(magnitude, 2.0 * square / (1.0 - magnitude))
            .ln_1p()
    } else {
        0.5 * (2.0 * magnitude / (1.0 - magnitude)).ln_1p()
    };
    result.copysign(value)
}

impl Context {
    pub(in crate::runtime::native) fn math_object_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(MATH_NAME) {
            let value = binding.value(MATH_NAME)?;
            self.define_math_global_property(value.clone())?;
            return Ok(value);
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
        self.define_math_to_string_tag(object)?;

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
        self.define_math_method(object, MATH_F16ROUND_NAME, NativeFunctionKind::MathF16round)?;
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
        self.define_math_method(
            object,
            MATH_SUM_PRECISE_NAME,
            NativeFunctionKind::MathSumPrecise,
        )?;
        self.define_math_method(object, MATH_TAN_NAME, NativeFunctionKind::MathTan)?;
        self.define_math_method(object, MATH_TANH_NAME, NativeFunctionKind::MathTanh)?;
        self.define_math_method(object, MATH_TRUNC_NAME, NativeFunctionKind::MathTrunc)?;

        let value = Value::Object(object);
        self.insert_global_builtin(MATH_NAME, value.clone())?;
        self.define_math_global_property(value.clone())?;
        Ok(value)
    }

    math_unary_method!(eval_math_abs, eval_direct_math_abs, f64::abs);
    math_unary_method!(eval_math_acos, eval_direct_math_acos, f64::acos);
    math_unary_method!(eval_math_acosh, eval_direct_math_acosh, f64::acosh);
    math_unary_method!(eval_math_asin, eval_direct_math_asin, f64::asin);
    math_unary_method!(eval_math_asinh, eval_direct_math_asinh, f64::asinh);
    math_unary_method!(eval_math_atan, eval_direct_math_atan, f64::atan);

    pub(in crate::runtime::native) fn eval_math_atan2(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_atan2(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_atan2(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let (y, x) = Self::eval_math_binary_values(args);
        let y = self.math_arg_or_nan(y)?;
        let x = self.math_arg_or_nan(x)?;
        Self::math_number(y.atan2(x))
    }

    math_unary_method!(eval_math_atanh, eval_direct_math_atanh, accurate_atanh);
    math_unary_method!(eval_math_cbrt, eval_direct_math_cbrt, f64::cbrt);
    math_unary_method!(eval_math_ceil, eval_direct_math_ceil, f64::ceil);

    pub(in crate::runtime::native) fn eval_math_clz32(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_clz32(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_clz32(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let unsigned = self.math_uint32_arg_or_zero(args.first(), MATH_CLZ32_NAME)?;
        Self::math_number(f64::from(unsigned.leading_zeros()))
    }

    math_unary_method!(eval_math_cos, eval_direct_math_cos, f64::cos);
    math_unary_method!(eval_math_cosh, eval_direct_math_cosh, f64::cosh);
    math_unary_method!(eval_math_exp, eval_direct_math_exp, f64::exp);
    math_unary_method!(eval_math_expm1, eval_direct_math_expm1, f64::exp_m1);

    pub(in crate::runtime::native) fn eval_math_f16round(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_f16round(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_f16round(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let number = self.math_arg_or_nan(args.first())?;
        Self::math_number(Self::f16round_to_number(number))
    }

    math_unary_method!(eval_math_floor, eval_direct_math_floor, f64::floor);

    pub(in crate::runtime::native) fn eval_math_fround(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_fround(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_fround(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let number = self.math_arg_or_nan(args.first())?;
        Self::math_number(Self::fround_to_number(number))
    }

    pub(in crate::runtime::native) fn eval_math_hypot(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_hypot(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_hypot(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        self.eval_math_hypot_values(args)
    }

    pub(in crate::runtime::native) fn eval_math_imul(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_imul(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_imul(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let (left, right) = Self::eval_math_binary_values(args);
        let left = self.math_uint32_arg_or_zero(left, MATH_IMUL_NAME)?;
        let right = self.math_uint32_arg_or_zero(right, MATH_IMUL_NAME)?;
        let product = left.wrapping_mul(right);
        Self::math_number(f64::from(i32::from_ne_bytes(product.to_ne_bytes())))
    }

    math_unary_method!(eval_math_log, eval_direct_math_log, f64::ln);
    math_unary_method!(eval_math_log10, eval_direct_math_log10, f64::log10);
    math_unary_method!(eval_math_log1p, eval_direct_math_log1p, f64::ln_1p);
    math_unary_method!(eval_math_log2, eval_direct_math_log2, f64::log2);

    pub(in crate::runtime::native) fn eval_math_max(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_max(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_max(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let mut maximum = f64::NEG_INFINITY;
        let mut has_nan = false;
        for value in args {
            let value = self.math_to_number(value)?;
            if value.is_nan() {
                has_nan = true;
            } else if value > maximum || Self::should_replace_max_zero(maximum, value) {
                maximum = value;
            }
        }
        if has_nan {
            return Self::math_number(f64::NAN);
        }
        Self::math_number(maximum)
    }

    pub(in crate::runtime::native) fn eval_math_min(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_min(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_min(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let mut minimum = f64::INFINITY;
        let mut has_nan = false;
        for value in args {
            let value = self.math_to_number(value)?;
            if value.is_nan() {
                has_nan = true;
            } else if value < minimum || Self::should_replace_min_zero(minimum, value) {
                minimum = value;
            }
        }
        if has_nan {
            return Self::math_number(f64::NAN);
        }
        Self::math_number(minimum)
    }

    pub(in crate::runtime::native) fn eval_math_pow(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_pow(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_pow(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let (base, exponent) = Self::eval_math_binary_values(args);
        let base = self.math_arg_or_nan(base)?;
        let exponent = self.math_arg_or_nan(exponent)?;
        Self::math_number(number_exponentiate(base, exponent))
    }

    pub(in crate::runtime::native) fn eval_math_random(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_random(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_random(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        Self::eval_math_discard_values(args);
        Ok(Value::Number(self.next_math_random()?))
    }

    pub(in crate::runtime::native) fn eval_math_round(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_round(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_round(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let value = args.first();
        Self::math_number(Self::round_to_nearest_toward_positive(
            self.math_arg_or_nan(value)?,
        ))
    }

    pub(in crate::runtime::native) fn eval_math_sign(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_sign(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_sign(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let value = args.first();
        let value = self.math_arg_or_nan(value)?;
        if value.is_nan() || value == 0.0 {
            return Self::math_number(value);
        }
        if value.is_sign_negative() {
            return Self::math_number(-1.0);
        }
        Self::math_number(1.0)
    }

    math_unary_method!(eval_math_sin, eval_direct_math_sin, f64::sin);
    math_unary_method!(eval_math_sinh, eval_direct_math_sinh, f64::sinh);
    math_unary_method!(eval_math_sqrt, eval_direct_math_sqrt, f64::sqrt);

    pub(in crate::runtime::native) fn eval_math_sum_precise(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_math_sum_precise(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_math_sum_precise(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let Some(iterable) = args.first() else {
            return Err(Error::type_error("Math.sumPrecise requires an iterable"));
        };
        let mut source = self.get_iterator(iterable)?;
        let mut sum = PreciseFiniteSum::new();
        loop {
            self.step()?;
            match self.iterator_step(&mut source)? {
                IteratorStep::Value(value) => {
                    if let Err(error) = sum.add_value(&value) {
                        return Err(self.iterator_close_on_error(&mut source, error));
                    }
                }
                IteratorStep::Done => return Self::math_number(sum.finish()?),
                IteratorStep::Abrupt(completion) => return completion.into_result(),
            }
        }
    }

    math_unary_method!(eval_math_tan, eval_direct_math_tan, f64::tan);
    math_unary_method!(eval_math_tanh, eval_direct_math_tanh, f64::tanh);
    math_unary_method!(eval_math_trunc, eval_direct_math_trunc, f64::trunc);

    pub(in crate::runtime::native) fn eval_direct_math_integer_number_target(
        target: NativeCallTarget,
        args: &[Value],
    ) -> Option<Value> {
        match target {
            NativeCallTarget::MathClz32 => Self::eval_direct_math_clz32_number(args),
            NativeCallTarget::MathF16round => Self::eval_direct_math_f16round_number(args),
            NativeCallTarget::MathFround => Self::eval_direct_math_fround_number(args),
            NativeCallTarget::MathImul => Self::eval_direct_math_imul_number(args),
            _ => None,
        }
    }

    fn eval_direct_math_clz32_number(args: &[Value]) -> Option<Value> {
        let Some(Value::Number(value)) = args.first() else {
            return None;
        };
        let unsigned = Self::nonnegative_number_to_uint32_fast(*value)?;
        Some(Value::Number(f64::from(unsigned.leading_zeros())))
    }

    fn eval_direct_math_fround_number(args: &[Value]) -> Option<Value> {
        let Some(Value::Number(value)) = args.first() else {
            return None;
        };
        Some(Value::Number(Self::fround_to_number(*value)))
    }

    fn eval_direct_math_f16round_number(args: &[Value]) -> Option<Value> {
        let Some(Value::Number(value)) = args.first() else {
            return None;
        };
        Some(Value::Number(Self::f16round_to_number(*value)))
    }

    fn eval_direct_math_imul_number(args: &[Value]) -> Option<Value> {
        let Some(Value::Number(left)) = args.first() else {
            return None;
        };
        let Some(Value::Number(right)) = args.get(1) else {
            return None;
        };
        let left = Self::nonnegative_number_to_uint32_fast(*left)?;
        let right = Self::nonnegative_number_to_uint32_fast(*right)?;
        let product = left.wrapping_mul(right);
        Some(Value::Number(f64::from(i32::from_ne_bytes(
            product.to_ne_bytes(),
        ))))
    }

    fn define_math_constant(&mut self, object: ObjectId, name: &str, value: f64) -> Result<()> {
        let key = self.intern_property_key(name)?;
        self.objects.define_property(
            object,
            key,
            name,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Number(value)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )
    }

    fn define_math_to_string_tag(&mut self, object: ObjectId) -> Result<()> {
        let value = self.heap_string_value(MATH_NAME)?;
        let key = self.math_well_known_symbol_property_key(SYMBOL_TO_STRING_TAG_PROPERTY)?;
        self.objects.define_property(
            object,
            key,
            SYMBOL_TO_STRING_TAG_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn define_math_global_property(&mut self, value: Value) -> Result<()> {
        let global = self.global_object_id()?;
        self.define_global_object_data_property(
            global,
            MATH_NAME,
            value,
            PropertyWritable::Yes,
            PropertyEnumerable::No,
            PropertyConfigurable::Yes,
        )
    }

    fn math_well_known_symbol_property_key(&mut self, property: &str) -> Result<PropertyKey> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("well-known Symbol property is not a symbol"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
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

    fn eval_math_unary_value(
        &mut self,
        value: Option<&Value>,
        operation: fn(f64) -> f64,
    ) -> Result<Value> {
        Self::math_number(operation(self.math_arg_or_nan(value)?))
    }

    fn eval_math_binary_values(args: &[Value]) -> (Option<&Value>, Option<&Value>) {
        (args.first(), args.get(1))
    }

    fn eval_math_hypot_values(&mut self, args: &[Value]) -> Result<Value> {
        let mut has_infinity = false;
        let mut has_nan = false;
        let mut magnitude = 0.0_f64;
        for value in args {
            let value = self.math_to_number(value)?;
            if value.is_infinite() {
                has_infinity = true;
            } else if value.is_nan() {
                has_nan = true;
            } else {
                magnitude = magnitude.hypot(value);
            }
        }
        if has_infinity {
            return Self::math_number(f64::INFINITY);
        }
        if has_nan {
            return Self::math_number(f64::NAN);
        }
        Self::math_number(magnitude)
    }

    fn math_to_number(&mut self, value: &Value) -> Result<f64> {
        self.to_number(value)
    }

    const fn eval_math_discard_values(_args: &[Value]) {}

    fn math_arg_or_nan(&mut self, value: Option<&Value>) -> Result<f64> {
        value.map_or(Ok(f64::NAN), |value| self.to_number(value))
    }

    fn math_uint32_arg_or_zero(&mut self, value: Option<&Value>, context: &str) -> Result<u32> {
        let Some(value) = value else {
            return Ok(0);
        };
        match value {
            Value::Number(value) => Self::math_number_to_uint32(*value, context),
            value => number_to_uint32(self.to_number(value)?, context),
        }
    }

    fn math_number_to_uint32(value: f64, context: &str) -> Result<u32> {
        if let Some(unsigned) = Self::nonnegative_number_to_uint32_fast(value) {
            return Ok(unsigned);
        }
        number_to_uint32(value, context)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn nonnegative_number_to_uint32_fast(value: f64) -> Option<u32> {
        if value.is_finite() && (0.0..=f64::from(u32::MAX)).contains(&value) {
            return Some(value as u32);
        }
        None
    }

    // Native function dispatch requires Result<Value>; number creation cannot fail.
    #[allow(clippy::unnecessary_wraps)]
    const fn math_number(value: f64) -> Result<Value> {
        Ok(Value::Number(value))
    }

    fn fround_to_number(value: f64) -> f64 {
        f64::from(Self::round_to_binary32(value))
    }

    fn f16round_to_number(value: f64) -> f64 {
        if value.is_nan() || value.is_infinite() || value == 0.0 {
            return value;
        }

        let sign = value.is_sign_negative();
        let abs = value.abs();
        let rounded = if abs >= F16_OVERFLOW_CUTOFF {
            f64::INFINITY
        } else if abs > F16_MAX_FINITE {
            F16_MAX_FINITE
        } else if abs <= F16_HALF_MIN_SUBNORMAL {
            0.0
        } else if abs < F16_MIN_NORMAL {
            let units = Self::round_ties_to_even(abs / F16_MIN_SUBNORMAL);
            units * F16_MIN_SUBNORMAL
        } else {
            Self::round_normal_binary16(abs)
        };

        if sign { -rounded } else { rounded }
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
        if value.abs() >= ROUND_INTEGER_CUTOFF {
            return value;
        }
        if value > 0.0 && value < 0.5 {
            return 0.0;
        }
        if (-0.5..=0.0).contains(&value) {
            return -0.0;
        }
        let rounded = (value + 0.5).floor();
        if rounded == 0.0 && value.is_sign_negative() {
            return -0.0;
        }
        rounded
    }

    fn round_normal_binary16(value: f64) -> f64 {
        let exponent = value.log2().floor();
        let quantum = (exponent - f64::from(F16_SIGNIFICAND_BITS)).exp2();
        let rounded_units = Self::round_ties_to_even(value / quantum);
        let rounded = rounded_units * quantum;
        if rounded >= F16_OVERFLOW_CUTOFF {
            f64::INFINITY
        } else {
            rounded
        }
    }

    fn round_ties_to_even(value: f64) -> f64 {
        let lower = value.floor();
        let fraction = value - lower;
        if fraction < 0.5 {
            return lower;
        }
        if fraction > 0.5 || !Self::is_even_integer(lower) {
            return lower + 1.0;
        }
        lower
    }

    fn is_even_integer(value: f64) -> bool {
        (value / 2.0).fract() == 0.0
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
