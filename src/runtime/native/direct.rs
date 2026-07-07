use crate::{
    api::native_call::NativeCallTarget,
    error::Result,
    runtime::{
        Context,
        call_args::RuntimeCallArgs,
        object::{CacheableNativePropertyValue, PropertyLookup},
        property::DynamicPropertyKey,
    },
    syntax::{StaticName, StaticPropertyAccessId},
    value::{NativeFunctionId, Value},
};

use super::NativeFunctionKind;

const PROTOTYPE_PROPERTY: &str = "__proto__";

const fn runtime_call_args(args: &[Value]) -> RuntimeCallArgs<'_> {
    RuntimeCallArgs::values(args)
}

impl Context {
    pub(crate) fn eval_direct_native_property_call(
        &mut self,
        target: NativeCallTarget,
        access: StaticPropertyAccessId,
        callee: Value,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if let Value::NativeFunction(id) = callee {
            if self
                .cached_static_property_native_call_kind(access, id)?
                .is_some()
            {
                self.record_native_call_cache_hit();
                return self.eval_direct_native_call_target(target, args, this_value);
            }
            if let Some(kind) = self.direct_native_call_kind(id, target) {
                self.record_native_call_cache_miss();
                self.remember_static_property_native_call_kind(access, id, kind)?;
                return self.eval_direct_native_call_target(target, args, this_value);
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

        if self
            .cached_static_object_property_native_call_kind_for_access(access, *object)?
            .is_some()
        {
            self.record_native_call_cache_hit();
            return self
                .eval_direct_native_call_target(target, args, this_value)
                .map(Some);
        }

        let lookup = self.static_property_lookup(property)?;
        if self
            .cached_static_object_property_native_call_kind(access, *object, lookup)?
            .is_some()
        {
            self.record_native_call_cache_hit();
            return self
                .eval_direct_native_call_target(target, args, this_value)
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
                        .eval_direct_native_call_target(target, args, this_value)
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

    pub(crate) fn eval_cached_native_dynamic_member_call(
        &mut self,
        property: &DynamicPropertyKey,
        access: StaticPropertyAccessId,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Option<Value>> {
        let Value::Object(object) = this_value else {
            return Ok(None);
        };
        if property.name() == PROTOTYPE_PROPERTY
            || self.objects.array_len_if_array(*object)?.is_some()
        {
            return Ok(None);
        }

        let lookup = property.lookup();
        if let Some(kind) =
            self.cached_static_object_property_native_call_kind(access, *object, lookup)?
        {
            self.record_native_call_cache_hit();
            return self
                .eval_direct_or_generic_native_function_kind(kind, args, this_value)
                .map(Some);
        }

        let candidate = self
            .objects
            .cacheable_property_lookup(*object, property.lookup())?;
        match self
            .objects
            .read_cacheable_native_property_value_for(*object, candidate)?
        {
            CacheableNativePropertyValue::Native { function, version } => {
                let kind = self.native_function(function)?.kind();
                self.record_native_call_cache_miss();
                self.remember_static_object_property_native_call_kind(
                    access, candidate, version, function, kind,
                )?;
                self.eval_direct_or_generic_native_function_kind(kind, args, this_value)
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

    pub(in crate::runtime) fn eval_direct_native_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        match target {
            NativeCallTarget::Array => self.eval_direct_array_constructor(args),
            NativeCallTarget::ArrayConcat => self.eval_direct_array_concat(args, this_value),
            NativeCallTarget::ArrayIncludes => self.eval_direct_array_includes(args, this_value),
            NativeCallTarget::ArrayIndexOf => self.eval_direct_array_index_of(args, this_value),
            NativeCallTarget::ArrayIsArray => self.eval_direct_array_is_array(args),
            NativeCallTarget::ArrayJoin => self.eval_direct_array_join(args, this_value),
            NativeCallTarget::ArrayLastIndexOf => {
                self.eval_direct_array_last_index_of(args, this_value)
            }
            NativeCallTarget::ArrayPop => self.eval_direct_array_pop(args, this_value),
            NativeCallTarget::ArrayPush => self.eval_direct_array_push(args, this_value),
            NativeCallTarget::ArrayReverse => self.eval_direct_array_reverse(args, this_value),
            NativeCallTarget::ArrayShift => self.eval_direct_array_shift(args, this_value),
            NativeCallTarget::ArraySlice => self.eval_direct_array_slice(args, this_value),
            NativeCallTarget::ArrayUnshift => self.eval_direct_array_unshift(args, this_value),
            NativeCallTarget::Boolean => self.eval_direct_boolean_constructor(args),
            NativeCallTarget::ErrorConstructor(name) => {
                self.eval_direct_error_constructor(name, args)
            }
            NativeCallTarget::Function => self.eval_direct_function_constructor(args),
            NativeCallTarget::JsonParse => self.eval_direct_json_parse(args),
            NativeCallTarget::JsonStringify => self.eval_direct_json_stringify(args),
            NativeCallTarget::MathAbs => Self::eval_direct_math_abs(args),
            NativeCallTarget::MathAcos => Self::eval_direct_math_acos(args),
            NativeCallTarget::MathAcosh => Self::eval_direct_math_acosh(args),
            NativeCallTarget::MathAsin => Self::eval_direct_math_asin(args),
            NativeCallTarget::MathAsinh => Self::eval_direct_math_asinh(args),
            NativeCallTarget::MathAtan => Self::eval_direct_math_atan(args),
            NativeCallTarget::MathAtan2 => Self::eval_direct_math_atan2(args),
            NativeCallTarget::MathAtanh => Self::eval_direct_math_atanh(args),
            NativeCallTarget::MathCbrt => Self::eval_direct_math_cbrt(args),
            NativeCallTarget::MathCeil => Self::eval_direct_math_ceil(args),
            NativeCallTarget::MathClz32 => Self::eval_direct_math_clz32(args),
            NativeCallTarget::MathCos => Self::eval_direct_math_cos(args),
            NativeCallTarget::MathCosh => Self::eval_direct_math_cosh(args),
            NativeCallTarget::MathExp => Self::eval_direct_math_exp(args),
            NativeCallTarget::MathExpm1 => Self::eval_direct_math_expm1(args),
            NativeCallTarget::MathFloor => Self::eval_direct_math_floor(args),
            NativeCallTarget::MathFround => Self::eval_direct_math_fround(args),
            NativeCallTarget::MathHypot => Self::eval_direct_math_hypot(args),
            NativeCallTarget::MathImul => Self::eval_direct_math_imul(args),
            NativeCallTarget::MathLog => Self::eval_direct_math_log(args),
            NativeCallTarget::MathLog10 => Self::eval_direct_math_log10(args),
            NativeCallTarget::MathLog1p => Self::eval_direct_math_log1p(args),
            NativeCallTarget::MathLog2 => Self::eval_direct_math_log2(args),
            NativeCallTarget::MathMax => Self::eval_direct_math_max(args),
            NativeCallTarget::MathMin => Self::eval_direct_math_min(args),
            NativeCallTarget::MathPow => Self::eval_direct_math_pow(args),
            NativeCallTarget::MathRandom => self.eval_direct_math_random(args),
            NativeCallTarget::MathRound => Self::eval_direct_math_round(args),
            NativeCallTarget::MathSign => Self::eval_direct_math_sign(args),
            NativeCallTarget::MathSin => Self::eval_direct_math_sin(args),
            NativeCallTarget::MathSinh => Self::eval_direct_math_sinh(args),
            NativeCallTarget::MathSqrt => Self::eval_direct_math_sqrt(args),
            NativeCallTarget::MathTan => Self::eval_direct_math_tan(args),
            NativeCallTarget::MathTanh => Self::eval_direct_math_tanh(args),
            NativeCallTarget::MathTrunc => Self::eval_direct_math_trunc(args),
            NativeCallTarget::Number => self.eval_direct_number_constructor(args),
            NativeCallTarget::Object => self.eval_direct_object_constructor(args),
            NativeCallTarget::ObjectDefineProperty => {
                self.eval_object_define_property(runtime_call_args(args))
            }
            NativeCallTarget::ObjectGetOwnPropertyDescriptor => {
                self.eval_object_get_own_property_descriptor(runtime_call_args(args))
            }
            NativeCallTarget::ObjectGetPrototypeOf => {
                self.eval_object_get_prototype_of(runtime_call_args(args))
            }
            NativeCallTarget::ObjectHasOwn => self.eval_object_has_own(runtime_call_args(args)),
            NativeCallTarget::ObjectKeys => self.eval_object_keys(runtime_call_args(args)),
            NativeCallTarget::Promise => self.eval_direct_promise_constructor(args),
            NativeCallTarget::PromiseResolve => self.eval_direct_promise_resolve(args),
            NativeCallTarget::PromiseReject => self.eval_direct_promise_reject(args),
            NativeCallTarget::PromiseThen => self.eval_direct_promise_then(args, this_value),
            NativeCallTarget::PromiseCatch => self.eval_direct_promise_catch(args, this_value),
            NativeCallTarget::String => self.eval_direct_string_constructor(args),
        }
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

    pub(in crate::runtime) fn eval_direct_or_generic_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if let Some(target) = kind.to_call_target() {
            return self.eval_direct_native_call_target(target, args, this_value);
        }
        self.eval_native_function_kind(kind, runtime_call_args(args), this_value)
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
            NativeFunctionKind::ArrayIsArray => self.eval_array_is_array(args),
            NativeFunctionKind::ArrayJoin => self.eval_array_join(args, this_value),
            NativeFunctionKind::ArrayLastIndexOf => self.eval_array_last_index_of(args, this_value),
            NativeFunctionKind::ArrayPop => self.eval_array_pop(args, this_value),
            NativeFunctionKind::ArrayPush => self.eval_array_push(args, this_value),
            NativeFunctionKind::ArrayReverse => self.eval_array_reverse(args, this_value),
            NativeFunctionKind::ArrayShift => self.eval_array_shift(args, this_value),
            NativeFunctionKind::ArraySlice => self.eval_array_slice(args, this_value),
            NativeFunctionKind::ArrayUnshift => self.eval_array_unshift(args, this_value),
            NativeFunctionKind::AsyncFunction => self.eval_async_function_constructor(args),
            NativeFunctionKind::Boolean => self.eval_boolean_constructor(args),
            NativeFunctionKind::BoundFunction(id) => self.eval_bound_function(id, args),
            NativeFunctionKind::Eval => self.eval_eval_function(args),
            NativeFunctionKind::ErrorConstructor(name) => self.eval_error_constructor(name, args),
            NativeFunctionKind::Function => self.eval_function_constructor(args),
            NativeFunctionKind::FunctionPrototypeBind => {
                self.eval_function_prototype_bind(args, this_value)
            }
            NativeFunctionKind::FunctionPrototypeCall => {
                self.eval_function_prototype_call(args, this_value)
            }
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
            NativeFunctionKind::ObjectGetOwnPropertyNames => {
                self.eval_object_get_own_property_names(args)
            }
            NativeFunctionKind::ObjectGetPrototypeOf => self.eval_object_get_prototype_of(args),
            NativeFunctionKind::ObjectHasOwn => self.eval_object_has_own(args),
            NativeFunctionKind::ObjectKeys => self.eval_object_keys(args),
            NativeFunctionKind::ObjectPrototypeHasOwnProperty => {
                self.eval_object_prototype_has_own_property(args, this_value)
            }
            NativeFunctionKind::ObjectPrototypePropertyIsEnumerable => {
                self.eval_object_prototype_property_is_enumerable(args, this_value)
            }
            NativeFunctionKind::Promise => self.eval_promise_constructor(args),
            NativeFunctionKind::PromiseResolve => self.eval_promise_resolve(args),
            NativeFunctionKind::PromiseReject => self.eval_promise_reject(args),
            NativeFunctionKind::PromiseThen => self.eval_promise_then(args, this_value),
            NativeFunctionKind::PromiseCatch => self.eval_promise_catch(args, this_value),
            NativeFunctionKind::PromiseResolver { promise, kind } => {
                self.eval_promise_resolver(promise, kind, args)
            }
            NativeFunctionKind::String => self.eval_string_constructor(args),
            NativeFunctionKind::Symbol => self.eval_symbol_constructor(args),
        }
    }
}
