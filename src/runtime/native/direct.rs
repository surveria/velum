use crate::{
    api::native_call::NativeCallTarget,
    ast::{StaticName, StaticPropertyAccessId},
    error::Result,
    runtime::{
        Context,
        call_args::RuntimeCallArgs,
        object::{CacheableNativePropertyValue, PropertyLookup},
    },
    value::{NativeFunctionId, Value},
};

use super::NativeFunctionKind;

const PROTOTYPE_PROPERTY: &str = "__proto__";

impl Context {
    pub(crate) fn eval_native_function(
        &mut self,
        id: NativeFunctionId,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let kind = self.native_function(id)?.kind();
        self.eval_native_function_kind(kind, args, this_value)
    }

    pub(crate) fn eval_direct_native_property_call(
        &mut self,
        target: NativeCallTarget,
        access: StaticPropertyAccessId,
        callee: Value,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if let Value::NativeFunction(id) = callee {
            if let Some(kind) = self.cached_static_property_native_call_kind(access, id)? {
                self.record_native_call_cache_hit();
                return self.eval_direct_native_function_kind(target, kind, args, this_value);
            }
            if let Some(kind) = self.direct_native_call_kind(id, target) {
                self.record_native_call_cache_miss();
                self.remember_static_property_native_call_kind(access, id, kind)?;
                return self.eval_direct_native_function_kind(target, kind, args, this_value);
            }
        }
        self.record_native_call_cache_fallback();
        self.eval_call_value(callee, args, this_value.clone())
    }

    pub(crate) fn eval_cached_direct_native_static_member_call(
        &mut self,
        target: NativeCallTarget,
        property: &StaticName,
        access: StaticPropertyAccessId,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Option<Value>> {
        let Value::Object(object) = this_value else {
            return Ok(None);
        };
        if property.as_str() == PROTOTYPE_PROPERTY {
            return Ok(None);
        }

        let lookup = self.static_property_lookup(property)?;
        if let Some(kind) =
            self.cached_static_object_property_native_call_kind(access, *object, lookup)?
        {
            self.record_native_call_cache_hit();
            return self
                .eval_direct_native_function_kind(target, kind, args, this_value)
                .map(Some);
        }

        let Some(keyed_lookup) = lookup
            .key()
            .map(|key| PropertyLookup::from_key(lookup.name(), key))
        else {
            return Ok(None);
        };
        let candidate = self
            .objects
            .cacheable_property_lookup(*object, keyed_lookup)?;
        match self
            .objects
            .read_cacheable_native_property_value_for(*object, candidate)?
        {
            CacheableNativePropertyValue::Native { function, version } => {
                if let Some(kind) = self.direct_native_call_kind(function, target) {
                    self.record_native_call_cache_miss();
                    self.remember_static_object_property_native_call_kind(
                        access, candidate, version, function, kind,
                    )?;
                    return self
                        .eval_direct_native_function_kind(target, kind, args, this_value)
                        .map(Some);
                }
                self.record_native_call_cache_fallback();
                self.eval_call_value(Value::NativeFunction(function), args, this_value.clone())
                    .map(Some)
            }
            CacheableNativePropertyValue::Other(callee) => {
                self.record_native_call_cache_fallback();
                self.eval_call_value(callee, args, this_value.clone())
                    .map(Some)
            }
            CacheableNativePropertyValue::Missing => {
                self.record_native_call_cache_fallback();
                self.eval_call_value(Value::Undefined, args, this_value.clone())
                    .map(Some)
            }
            CacheableNativePropertyValue::Uncacheable => Ok(None),
        }
    }

    fn eval_direct_native_function_kind(
        &mut self,
        target: NativeCallTarget,
        kind: NativeFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if target.is_array_target() {
            return self.eval_array_native_function_kind(
                kind,
                RuntimeCallArgs::values(args),
                this_value,
            );
        }
        self.eval_native_function_kind(kind, RuntimeCallArgs::values(args), this_value)
    }

    pub(in crate::runtime) fn direct_native_call_kind(
        &self,
        id: NativeFunctionId,
        target: NativeCallTarget,
    ) -> Option<NativeFunctionKind> {
        let kind = NativeFunctionKind::from_call_target(target);
        self.native_function_id(kind)
            .filter(|expected| *expected == id)
            .map(|_| kind)
    }

    pub(in crate::runtime) fn eval_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match kind {
            NativeFunctionKind::Array => self.eval_array_constructor(args),
            NativeFunctionKind::ArrayConcat => self.eval_array_concat(args, this_value),
            NativeFunctionKind::ArrayIncludes => self.eval_array_includes(args, this_value),
            NativeFunctionKind::ArrayIndexOf => self.eval_array_index_of(args, this_value),
            NativeFunctionKind::ArrayJoin => self.eval_array_join(args, this_value),
            NativeFunctionKind::ArrayLastIndexOf => self.eval_array_last_index_of(args, this_value),
            NativeFunctionKind::ArrayPop => self.eval_array_pop(args, this_value),
            NativeFunctionKind::ArrayPush => self.eval_array_push(args, this_value),
            NativeFunctionKind::ArrayReverse => self.eval_array_reverse(args, this_value),
            NativeFunctionKind::ArrayShift => self.eval_array_shift(args, this_value),
            NativeFunctionKind::ArraySlice => self.eval_array_slice(args, this_value),
            NativeFunctionKind::ArrayUnshift => self.eval_array_unshift(args, this_value),
            NativeFunctionKind::Boolean => self.eval_boolean_constructor(args),
            NativeFunctionKind::Eval => self.eval_eval_function(args),
            NativeFunctionKind::ErrorConstructor(name) => self.eval_error_constructor(name, args),
            NativeFunctionKind::JsonParse => self.eval_json_parse(args),
            NativeFunctionKind::JsonStringify => self.eval_json_stringify(args),
            NativeFunctionKind::MathAbs => Self::eval_math_abs(args),
            NativeFunctionKind::MathAcos => Self::eval_math_acos(args),
            NativeFunctionKind::MathAcosh => Self::eval_math_acosh(args),
            NativeFunctionKind::MathAsin => Self::eval_math_asin(args),
            NativeFunctionKind::MathAsinh => Self::eval_math_asinh(args),
            NativeFunctionKind::MathAtan => Self::eval_math_atan(args),
            NativeFunctionKind::MathAtan2 => Self::eval_math_atan2(args),
            NativeFunctionKind::MathAtanh => Self::eval_math_atanh(args),
            NativeFunctionKind::MathCbrt => Self::eval_math_cbrt(args),
            NativeFunctionKind::MathCeil => Self::eval_math_ceil(args),
            NativeFunctionKind::MathClz32 => Self::eval_math_clz32(args),
            NativeFunctionKind::MathCos => Self::eval_math_cos(args),
            NativeFunctionKind::MathCosh => Self::eval_math_cosh(args),
            NativeFunctionKind::MathExp => Self::eval_math_exp(args),
            NativeFunctionKind::MathExpm1 => Self::eval_math_expm1(args),
            NativeFunctionKind::MathFloor => Self::eval_math_floor(args),
            NativeFunctionKind::MathFround => Self::eval_math_fround(args),
            NativeFunctionKind::MathHypot => Self::eval_math_hypot(args),
            NativeFunctionKind::MathImul => Self::eval_math_imul(args),
            NativeFunctionKind::MathLog => Self::eval_math_log(args),
            NativeFunctionKind::MathLog10 => Self::eval_math_log10(args),
            NativeFunctionKind::MathLog1p => Self::eval_math_log1p(args),
            NativeFunctionKind::MathLog2 => Self::eval_math_log2(args),
            NativeFunctionKind::MathMax => Self::eval_math_max(args),
            NativeFunctionKind::MathMin => Self::eval_math_min(args),
            NativeFunctionKind::MathPow => Self::eval_math_pow(args),
            NativeFunctionKind::MathRandom => self.eval_math_random(args),
            NativeFunctionKind::MathRound => Self::eval_math_round(args),
            NativeFunctionKind::MathSign => Self::eval_math_sign(args),
            NativeFunctionKind::MathSin => Self::eval_math_sin(args),
            NativeFunctionKind::MathSinh => Self::eval_math_sinh(args),
            NativeFunctionKind::MathSqrt => Self::eval_math_sqrt(args),
            NativeFunctionKind::MathTan => Self::eval_math_tan(args),
            NativeFunctionKind::MathTanh => Self::eval_math_tanh(args),
            NativeFunctionKind::MathTrunc => Self::eval_math_trunc(args),
            NativeFunctionKind::Number => self.eval_number_constructor(args),
            NativeFunctionKind::Object => self.eval_object_constructor(args),
            NativeFunctionKind::ObjectDefineProperty => self.eval_object_define_property(args),
            NativeFunctionKind::ObjectGetOwnPropertyDescriptor => {
                self.eval_object_get_own_property_descriptor(args)
            }
            NativeFunctionKind::ObjectHasOwn => self.eval_object_has_own(args),
            NativeFunctionKind::ObjectKeys => self.eval_object_keys(args),
            NativeFunctionKind::Promise => self.eval_promise_constructor(args),
            NativeFunctionKind::PromiseResolve => self.eval_promise_resolve(args),
            NativeFunctionKind::PromiseReject => self.eval_promise_reject(args),
            NativeFunctionKind::PromiseThen => self.eval_promise_then(args, this_value),
            NativeFunctionKind::PromiseCatch => self.eval_promise_catch(args, this_value),
            NativeFunctionKind::PromiseResolver { promise, kind } => {
                self.eval_promise_resolver(promise, kind, args)
            }
            NativeFunctionKind::String => self.eval_string_constructor(args),
        }
    }
}
