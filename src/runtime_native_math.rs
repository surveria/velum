use std::f64::consts::{E, FRAC_1_SQRT_2, LN_2, LN_10, LOG2_E, LOG10_E, PI, SQRT_2};

use crate::{
    ast::Expr,
    error::Result,
    runtime::Context,
    value::{ObjectId, Value},
};

use super::{
    MATH_ABS_NAME, MATH_CEIL_NAME, MATH_FLOOR_NAME, MATH_MAX_NAME, MATH_MIN_NAME, MATH_NAME,
    MATH_POW_NAME, MATH_ROUND_NAME, MATH_SQRT_NAME, MATH_TRUNC_NAME, NativeFunctionKind,
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
        self.define_math_method(object, MATH_CEIL_NAME, NativeFunctionKind::MathCeil)?;
        self.define_math_method(object, MATH_FLOOR_NAME, NativeFunctionKind::MathFloor)?;
        self.define_math_method(object, MATH_MAX_NAME, NativeFunctionKind::MathMax)?;
        self.define_math_method(object, MATH_MIN_NAME, NativeFunctionKind::MathMin)?;
        self.define_math_method(object, MATH_POW_NAME, NativeFunctionKind::MathPow)?;
        self.define_math_method(object, MATH_ROUND_NAME, NativeFunctionKind::MathRound)?;
        self.define_math_method(object, MATH_SQRT_NAME, NativeFunctionKind::MathSqrt)?;
        self.define_math_method(object, MATH_TRUNC_NAME, NativeFunctionKind::MathTrunc)?;

        let value = Value::Object(object);
        self.insert_global_builtin(MATH_NAME, value.clone())?;
        Ok(value)
    }

    pub(super) fn eval_math_abs(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        Ok(Value::Number(Self::math_arg_or_nan(values.first()).abs()))
    }

    pub(super) fn eval_math_ceil(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        Ok(Value::Number(Self::math_arg_or_nan(values.first()).ceil()))
    }

    pub(super) fn eval_math_floor(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        Ok(Value::Number(Self::math_arg_or_nan(values.first()).floor()))
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

    pub(super) fn eval_math_sqrt(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        Ok(Value::Number(Self::math_arg_or_nan(values.first()).sqrt()))
    }

    pub(super) fn eval_math_trunc(&mut self, args: &[Expr]) -> Result<Value> {
        let values = self.eval_math_args(args)?;
        Ok(Value::Number(Self::math_arg_or_nan(values.first()).trunc()))
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

    fn math_arg_or_nan(value: Option<&Value>) -> f64 {
        value.map_or(f64::NAN, Self::value_to_number)
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
