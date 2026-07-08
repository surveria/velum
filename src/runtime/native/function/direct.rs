use crate::{
    api::native_call::NativeCallTarget,
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
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
            if let Some(kind) = self.cached_static_property_native_call_kind(access, id)? {
                self.record_native_call_cache_hit();
                return self.eval_direct_or_generic_native_function_kind(kind, args, this_value);
            }
            if let Some(kind) = self.direct_native_call_kind(id, target) {
                self.record_native_call_cache_miss();
                self.remember_static_property_native_call_kind(access, id, kind)?;
                return self.eval_direct_native_call_target(target, args, this_value);
            }
            let kind = self.native_function(id)?.kind();
            if kind.to_call_target().is_some() {
                self.record_native_call_cache_miss();
                self.remember_static_property_native_call_kind(access, id, kind)?;
                return self.eval_direct_or_generic_native_function_kind(kind, args, this_value);
            }
        }
        self.record_native_call_cache_slow_path();
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

        if let Some(kind) =
            self.cached_static_object_property_native_call_kind_for_access(access, *object)?
        {
            self.record_native_call_cache_hit();
            return self
                .eval_direct_or_generic_native_function_kind(kind, args, this_value)
                .map(Some);
        }

        let lookup = self.static_property_lookup(property)?;
        if let Some(kind) =
            self.cached_static_object_property_native_call_kind(access, *object, lookup)?
        {
            self.record_native_call_cache_hit();
            return self
                .eval_direct_or_generic_native_function_kind(kind, args, this_value)
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
                let kind = self.native_function(function)?.kind();
                if kind.to_call_target().is_some() {
                    self.record_native_call_cache_miss();
                    self.remember_static_object_property_native_call_kind(
                        access, candidate, version, function, kind,
                    )?;
                    return self
                        .eval_direct_or_generic_native_function_kind(kind, args, this_value)
                        .map(Some);
                }
                self.record_native_call_cache_slow_path();
                self.eval_call_value(Value::NativeFunction(function), args, this_value.clone())
                    .map(Some)
            }
            CacheableNativePropertyValue::Other(callee) => {
                self.record_native_call_cache_slow_path();
                self.eval_call_value(callee, args, this_value.clone())
                    .map(Some)
            }
            CacheableNativePropertyValue::Missing => {
                self.record_native_call_cache_slow_path();
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
                self.record_native_call_cache_slow_path();
                self.eval_call_value(callee, args, this_value.clone())
                    .map(Some)
            }
            CacheableNativePropertyValue::Missing => {
                self.record_native_call_cache_slow_path();
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
        if let Some(value) = Self::eval_direct_math_integer_number_target(target, args) {
            return Ok(value);
        }
        if let Some(value) = self.eval_direct_math_call_target(target, args) {
            return value;
        }
        if let Some(value) = self.eval_direct_global_utility_call_target(target, args) {
            return value;
        }
        if let Some(value) = self.eval_direct_object_native_call_target(target, args) {
            return value;
        }

        self.eval_direct_non_object_native_call_target(target, args, this_value)
    }

    fn eval_direct_non_object_native_call_target(
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
            NativeCallTarget::Eval => self.eval_eval_function(runtime_call_args(args)),
            NativeCallTarget::ErrorConstructor(name) => {
                self.eval_direct_error_constructor(name, args)
            }
            NativeCallTarget::ErrorPrototypeToString => {
                self.eval_direct_error_prototype_to_string(this_value)
            }
            NativeCallTarget::Function => self.eval_direct_function_constructor(args),
            NativeCallTarget::FunctionPrototypeBind => {
                self.eval_function_prototype_bind(runtime_call_args(args), this_value)
            }
            NativeCallTarget::FunctionPrototypeCall => {
                self.eval_function_prototype_call(runtime_call_args(args), this_value)
            }
            NativeCallTarget::JsonParse => self.eval_direct_json_parse(args),
            NativeCallTarget::JsonStringify => self.eval_direct_json_stringify(args),
            NativeCallTarget::Number => self.eval_direct_number_constructor(args),
            NativeCallTarget::Promise => self.eval_direct_promise_constructor(args),
            NativeCallTarget::PromiseResolve => self.eval_direct_promise_resolve(args),
            NativeCallTarget::PromiseReject => self.eval_direct_promise_reject(args),
            NativeCallTarget::PromiseThen => self.eval_direct_promise_then(args, this_value),
            NativeCallTarget::PromiseCatch => self.eval_direct_promise_catch(args, this_value),
            NativeCallTarget::RegExp => self.eval_direct_regexp_constructor(args),
            NativeCallTarget::RegExpPrototypeTest => {
                self.eval_regexp_prototype_test(runtime_call_args(args), this_value)
            }
            NativeCallTarget::Symbol => self.eval_symbol_constructor(runtime_call_args(args)),
            target => self
                .eval_direct_string_native_call_target(target, args, this_value)
                .ok_or_else(|| Error::runtime("String native call target was not handled"))?,
        }
    }

    fn eval_direct_string_native_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match target {
            NativeCallTarget::String => Some(self.eval_direct_string_constructor(args)),
            NativeCallTarget::StringFromCharCode => {
                Some(self.eval_direct_string_from_char_code(args))
            }
            NativeCallTarget::StringFromCodePoint => {
                Some(self.eval_direct_string_from_code_point(args))
            }
            NativeCallTarget::StringRaw => Some(self.eval_direct_string_raw(args)),
            NativeCallTarget::StringPrototypeAt => {
                Some(self.eval_direct_string_prototype_at(args, this_value))
            }
            NativeCallTarget::StringPrototypeCharAt => {
                Some(self.eval_direct_string_prototype_char_at(args, this_value))
            }
            NativeCallTarget::StringPrototypeCharCodeAt => {
                Some(self.eval_direct_string_prototype_char_code_at(args, this_value))
            }
            NativeCallTarget::StringPrototypeCodePointAt => {
                Some(self.eval_direct_string_prototype_code_point_at(args, this_value))
            }
            NativeCallTarget::StringPrototypeConcat => {
                Some(self.eval_direct_string_prototype_concat(args, this_value))
            }
            NativeCallTarget::StringPrototypeEndsWith => {
                Some(self.eval_direct_string_prototype_ends_with(args, this_value))
            }
            NativeCallTarget::StringPrototypeIncludes => {
                Some(self.eval_direct_string_prototype_includes(args, this_value))
            }
            NativeCallTarget::StringPrototypeIndexOf => {
                Some(self.eval_direct_string_prototype_index_of(args, this_value))
            }
            NativeCallTarget::StringPrototypeLastIndexOf => {
                Some(self.eval_direct_string_prototype_last_index_of(args, this_value))
            }
            NativeCallTarget::StringPrototypePadEnd => {
                Some(self.eval_string_prototype_pad_end(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypePadStart => {
                Some(self.eval_string_prototype_pad_start(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeRepeat => {
                Some(self.eval_direct_string_prototype_repeat(args, this_value))
            }
            NativeCallTarget::StringPrototypeSlice => {
                Some(self.eval_direct_string_prototype_slice(args, this_value))
            }
            NativeCallTarget::StringPrototypeStartsWith => {
                Some(self.eval_direct_string_prototype_starts_with(args, this_value))
            }
            NativeCallTarget::StringPrototypeSubstring => {
                Some(self.eval_direct_string_prototype_substring(args, this_value))
            }
            NativeCallTarget::StringPrototypeToLocaleLowerCase
            | NativeCallTarget::StringPrototypeToLowerCase => {
                Some(self.eval_string_prototype_to_lower_case(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeToLocaleUpperCase
            | NativeCallTarget::StringPrototypeToUpperCase => {
                Some(self.eval_string_prototype_to_upper_case(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeToString => {
                Some(self.eval_string_prototype_to_string(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeTrim => {
                Some(self.eval_string_prototype_trim(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeTrimEnd => {
                Some(self.eval_string_prototype_trim_end(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeTrimStart => {
                Some(self.eval_string_prototype_trim_start(runtime_call_args(args), this_value))
            }
            NativeCallTarget::StringPrototypeValueOf => {
                Some(self.eval_string_prototype_value_of(runtime_call_args(args), this_value))
            }
            _ => None,
        }
    }

    fn eval_direct_math_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
    ) -> Option<Result<Value>> {
        match target {
            NativeCallTarget::MathAbs => Some(Self::eval_direct_math_abs(args)),
            NativeCallTarget::MathAcos => Some(Self::eval_direct_math_acos(args)),
            NativeCallTarget::MathAcosh => Some(Self::eval_direct_math_acosh(args)),
            NativeCallTarget::MathAsin => Some(Self::eval_direct_math_asin(args)),
            NativeCallTarget::MathAsinh => Some(Self::eval_direct_math_asinh(args)),
            NativeCallTarget::MathAtan => Some(Self::eval_direct_math_atan(args)),
            NativeCallTarget::MathAtan2 => Some(Self::eval_direct_math_atan2(args)),
            NativeCallTarget::MathAtanh => Some(Self::eval_direct_math_atanh(args)),
            NativeCallTarget::MathCbrt => Some(Self::eval_direct_math_cbrt(args)),
            NativeCallTarget::MathCeil => Some(Self::eval_direct_math_ceil(args)),
            NativeCallTarget::MathClz32 => Some(Self::eval_direct_math_clz32(args)),
            NativeCallTarget::MathCos => Some(Self::eval_direct_math_cos(args)),
            NativeCallTarget::MathCosh => Some(Self::eval_direct_math_cosh(args)),
            NativeCallTarget::MathExp => Some(Self::eval_direct_math_exp(args)),
            NativeCallTarget::MathExpm1 => Some(Self::eval_direct_math_expm1(args)),
            NativeCallTarget::MathFloor => Some(Self::eval_direct_math_floor(args)),
            NativeCallTarget::MathFround => Some(Self::eval_direct_math_fround(args)),
            NativeCallTarget::MathHypot => Some(Self::eval_direct_math_hypot(args)),
            NativeCallTarget::MathImul => Some(Self::eval_direct_math_imul(args)),
            NativeCallTarget::MathLog => Some(Self::eval_direct_math_log(args)),
            NativeCallTarget::MathLog10 => Some(Self::eval_direct_math_log10(args)),
            NativeCallTarget::MathLog1p => Some(Self::eval_direct_math_log1p(args)),
            NativeCallTarget::MathLog2 => Some(Self::eval_direct_math_log2(args)),
            NativeCallTarget::MathMax => Some(Self::eval_direct_math_max(args)),
            NativeCallTarget::MathMin => Some(Self::eval_direct_math_min(args)),
            NativeCallTarget::MathPow => Some(Self::eval_direct_math_pow(args)),
            NativeCallTarget::MathRandom => Some(self.eval_direct_math_random(args)),
            NativeCallTarget::MathRound => Some(Self::eval_direct_math_round(args)),
            NativeCallTarget::MathSign => Some(Self::eval_direct_math_sign(args)),
            NativeCallTarget::MathSin => Some(Self::eval_direct_math_sin(args)),
            NativeCallTarget::MathSinh => Some(Self::eval_direct_math_sinh(args)),
            NativeCallTarget::MathSqrt => Some(Self::eval_direct_math_sqrt(args)),
            NativeCallTarget::MathTan => Some(Self::eval_direct_math_tan(args)),
            NativeCallTarget::MathTanh => Some(Self::eval_direct_math_tanh(args)),
            NativeCallTarget::MathTrunc => Some(Self::eval_direct_math_trunc(args)),
            _ => None,
        }
    }

    fn eval_direct_object_native_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
    ) -> Option<Result<Value>> {
        match target {
            NativeCallTarget::Object => Some(self.eval_direct_object_constructor(args)),
            NativeCallTarget::ObjectAssign => Some(self.eval_direct_object_assign(args)),
            NativeCallTarget::ObjectCreate => Some(self.eval_direct_object_create(args)),
            NativeCallTarget::ObjectDefineProperties => {
                Some(self.eval_object_define_properties(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectDefineProperty => {
                Some(self.eval_object_define_property(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectEntries => {
                Some(self.eval_object_entries(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectGetOwnPropertyDescriptor => {
                Some(self.eval_object_get_own_property_descriptor(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectGetOwnPropertyDescriptors => {
                Some(self.eval_object_get_own_property_descriptors(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectGetOwnPropertyNames => {
                Some(self.eval_object_get_own_property_names(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectGetPrototypeOf => {
                Some(self.eval_object_get_prototype_of(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectHasOwn => {
                Some(self.eval_object_has_own(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectIs => Some(Ok(Self::eval_direct_object_is(args))),
            NativeCallTarget::ObjectKeys => Some(self.eval_object_keys(runtime_call_args(args))),
            NativeCallTarget::ObjectSetPrototypeOf => {
                Some(self.eval_direct_object_set_prototype_of(args))
            }
            NativeCallTarget::ObjectValues => {
                Some(self.eval_object_values(runtime_call_args(args)))
            }
            _ => None,
        }
    }

    fn eval_direct_global_utility_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
    ) -> Option<Result<Value>> {
        match target {
            NativeCallTarget::GlobalDecodeUri => Some(self.eval_direct_global_decode_uri(args)),
            NativeCallTarget::GlobalDecodeUriComponent => {
                Some(self.eval_direct_global_decode_uri_component(args))
            }
            NativeCallTarget::GlobalEncodeUri => Some(self.eval_direct_global_encode_uri(args)),
            NativeCallTarget::GlobalEncodeUriComponent => {
                Some(self.eval_direct_global_encode_uri_component(args))
            }
            NativeCallTarget::GlobalIsFinite => Some(Ok(Self::eval_direct_global_is_finite(args))),
            NativeCallTarget::GlobalIsNan => Some(Ok(Self::eval_direct_global_is_nan(args))),
            NativeCallTarget::GlobalParseFloat => {
                Some(Ok(Self::eval_direct_global_parse_float(args)))
            }
            NativeCallTarget::GlobalParseInt => Some(Self::eval_direct_global_parse_int(args)),
            NativeCallTarget::NumberIsFinite => Some(Ok(Self::eval_direct_number_is_finite(args))),
            NativeCallTarget::NumberIsNan => Some(Ok(Self::eval_direct_number_is_nan(args))),
            _ => None,
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
        if let Some(value) = self.eval_global_utility_function_kind(kind, args) {
            return value;
        }
        if let Some(value) = self.eval_math_function_kind(kind, args) {
            return value;
        }
        if let Some(value) = self.eval_object_native_function_kind(kind, args, this_value) {
            return value;
        }

        self.eval_non_object_native_function_kind(kind, args, this_value)
    }

    fn eval_non_object_native_function_kind(
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
            NativeFunctionKind::ErrorPrototypeToString => {
                self.eval_error_prototype_to_string(args, this_value)
            }
            NativeFunctionKind::Function => self.eval_function_constructor(args),
            NativeFunctionKind::FunctionPrototypeBind => {
                self.eval_function_prototype_bind(args, this_value)
            }
            NativeFunctionKind::FunctionPrototypeCall => {
                self.eval_function_prototype_call(args, this_value)
            }
            NativeFunctionKind::JsonParse => self.eval_json_parse(args),
            NativeFunctionKind::JsonStringify => self.eval_json_stringify(args),
            NativeFunctionKind::Number => self.eval_number_constructor(args),
            NativeFunctionKind::Promise => self.eval_promise_constructor(args),
            NativeFunctionKind::PromiseResolve => self.eval_promise_resolve(args),
            NativeFunctionKind::PromiseReject => self.eval_promise_reject(args),
            NativeFunctionKind::PromiseThen => self.eval_promise_then(args, this_value),
            NativeFunctionKind::PromiseCatch => self.eval_promise_catch(args, this_value),
            NativeFunctionKind::PromiseResolver { promise, kind } => {
                self.eval_promise_resolver(promise, kind, args)
            }
            NativeFunctionKind::RegExp => self.eval_regexp_constructor(args),
            NativeFunctionKind::RegExpPrototypeTest => {
                self.eval_regexp_prototype_test(args, this_value)
            }
            NativeFunctionKind::Symbol => self.eval_symbol_constructor(args),
            kind => self
                .eval_string_native_function_kind(kind, args, this_value)
                .ok_or_else(|| Error::runtime("String native function kind was not handled"))?,
        }
    }

    fn eval_string_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::String => Some(self.eval_string_constructor(args)),
            NativeFunctionKind::StringFromCharCode => Some(self.eval_string_from_char_code(args)),
            NativeFunctionKind::StringFromCodePoint => Some(self.eval_string_from_code_point(args)),
            NativeFunctionKind::StringRaw => Some(self.eval_string_raw(args)),
            NativeFunctionKind::StringPrototypeAt => {
                Some(self.eval_string_prototype_at(args, this_value))
            }
            NativeFunctionKind::StringPrototypeCharAt => {
                Some(self.eval_string_prototype_char_at(args, this_value))
            }
            NativeFunctionKind::StringPrototypeCharCodeAt => {
                Some(self.eval_string_prototype_char_code_at(args, this_value))
            }
            NativeFunctionKind::StringPrototypeCodePointAt => {
                Some(self.eval_string_prototype_code_point_at(args, this_value))
            }
            NativeFunctionKind::StringPrototypeConcat => {
                Some(self.eval_string_prototype_concat(args, this_value))
            }
            NativeFunctionKind::StringPrototypeEndsWith => {
                Some(self.eval_string_prototype_ends_with(args, this_value))
            }
            NativeFunctionKind::StringPrototypeIncludes => {
                Some(self.eval_string_prototype_includes(args, this_value))
            }
            NativeFunctionKind::StringPrototypeIndexOf => {
                Some(self.eval_string_prototype_index_of(args, this_value))
            }
            NativeFunctionKind::StringPrototypeLastIndexOf => {
                Some(self.eval_string_prototype_last_index_of(args, this_value))
            }
            NativeFunctionKind::StringPrototypePadEnd => {
                Some(self.eval_string_prototype_pad_end(args, this_value))
            }
            NativeFunctionKind::StringPrototypePadStart => {
                Some(self.eval_string_prototype_pad_start(args, this_value))
            }
            NativeFunctionKind::StringPrototypeRepeat => {
                Some(self.eval_string_prototype_repeat(args, this_value))
            }
            NativeFunctionKind::StringPrototypeSlice => {
                Some(self.eval_string_prototype_slice(args, this_value))
            }
            NativeFunctionKind::StringPrototypeStartsWith => {
                Some(self.eval_string_prototype_starts_with(args, this_value))
            }
            NativeFunctionKind::StringPrototypeSubstring => {
                Some(self.eval_string_prototype_substring(args, this_value))
            }
            NativeFunctionKind::StringPrototypeToLocaleLowerCase
            | NativeFunctionKind::StringPrototypeToLowerCase => {
                Some(self.eval_string_prototype_to_lower_case(args, this_value))
            }
            NativeFunctionKind::StringPrototypeToLocaleUpperCase
            | NativeFunctionKind::StringPrototypeToUpperCase => {
                Some(self.eval_string_prototype_to_upper_case(args, this_value))
            }
            NativeFunctionKind::StringPrototypeToString => {
                Some(self.eval_string_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::StringPrototypeTrim => {
                Some(self.eval_string_prototype_trim(args, this_value))
            }
            NativeFunctionKind::StringPrototypeTrimEnd => {
                Some(self.eval_string_prototype_trim_end(args, this_value))
            }
            NativeFunctionKind::StringPrototypeTrimStart => {
                Some(self.eval_string_prototype_trim_start(args, this_value))
            }
            NativeFunctionKind::StringPrototypeValueOf => {
                Some(self.eval_string_prototype_value_of(args, this_value))
            }
            _ => None,
        }
    }

    fn eval_object_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::Object => Some(self.eval_object_constructor(args)),
            NativeFunctionKind::ObjectAssign => Some(self.eval_object_assign(args)),
            NativeFunctionKind::ObjectCreate => Some(self.eval_object_create(args)),
            NativeFunctionKind::ObjectDefineProperties => {
                Some(self.eval_object_define_properties(args))
            }
            NativeFunctionKind::ObjectDefineProperty => {
                Some(self.eval_object_define_property(args))
            }
            NativeFunctionKind::ObjectEntries => Some(self.eval_object_entries(args)),
            NativeFunctionKind::ObjectGetOwnPropertyDescriptor => {
                Some(self.eval_object_get_own_property_descriptor(args))
            }
            NativeFunctionKind::ObjectGetOwnPropertyDescriptors => {
                Some(self.eval_object_get_own_property_descriptors(args))
            }
            NativeFunctionKind::ObjectGetOwnPropertyNames => {
                Some(self.eval_object_get_own_property_names(args))
            }
            NativeFunctionKind::ObjectGetPrototypeOf => {
                Some(self.eval_object_get_prototype_of(args))
            }
            NativeFunctionKind::ObjectHasOwn => Some(self.eval_object_has_own(args)),
            NativeFunctionKind::ObjectIs => Some(Ok(Self::eval_direct_object_is(args.as_slice()))),
            NativeFunctionKind::ObjectKeys => Some(self.eval_object_keys(args)),
            NativeFunctionKind::ObjectPrototypeHasOwnProperty => {
                Some(self.eval_object_prototype_has_own_property(args, this_value))
            }
            NativeFunctionKind::ObjectPrototypePropertyIsEnumerable => {
                Some(self.eval_object_prototype_property_is_enumerable(args, this_value))
            }
            NativeFunctionKind::ObjectSetPrototypeOf => {
                Some(self.eval_object_set_prototype_of(args))
            }
            NativeFunctionKind::ObjectValues => Some(self.eval_object_values(args)),
            _ => None,
        }
    }

    fn eval_global_utility_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::GlobalDecodeUri => Some(self.eval_global_decode_uri(args)),
            NativeFunctionKind::GlobalDecodeUriComponent => {
                Some(self.eval_global_decode_uri_component(args))
            }
            NativeFunctionKind::GlobalEncodeUri => Some(self.eval_global_encode_uri(args)),
            NativeFunctionKind::GlobalEncodeUriComponent => {
                Some(self.eval_global_encode_uri_component(args))
            }
            NativeFunctionKind::GlobalIsFinite => Some(Ok(Self::eval_global_is_finite(args))),
            NativeFunctionKind::GlobalIsNan => Some(Ok(Self::eval_global_is_nan(args))),
            NativeFunctionKind::GlobalParseFloat => Some(Ok(Self::eval_global_parse_float(args))),
            NativeFunctionKind::GlobalParseInt => Some(Self::eval_global_parse_int(args)),
            NativeFunctionKind::NumberIsFinite => Some(Ok(Self::eval_number_is_finite(args))),
            NativeFunctionKind::NumberIsNan => Some(Ok(Self::eval_number_is_nan(args))),
            _ => None,
        }
    }

    fn eval_math_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::MathAbs => Some(Self::eval_math_abs(args)),
            NativeFunctionKind::MathAcos => Some(Self::eval_math_acos(args)),
            NativeFunctionKind::MathAcosh => Some(Self::eval_math_acosh(args)),
            NativeFunctionKind::MathAsin => Some(Self::eval_math_asin(args)),
            NativeFunctionKind::MathAsinh => Some(Self::eval_math_asinh(args)),
            NativeFunctionKind::MathAtan => Some(Self::eval_math_atan(args)),
            NativeFunctionKind::MathAtan2 => Some(Self::eval_math_atan2(args)),
            NativeFunctionKind::MathAtanh => Some(Self::eval_math_atanh(args)),
            NativeFunctionKind::MathCbrt => Some(Self::eval_math_cbrt(args)),
            NativeFunctionKind::MathCeil => Some(Self::eval_math_ceil(args)),
            NativeFunctionKind::MathClz32 => Some(Self::eval_math_clz32(args)),
            NativeFunctionKind::MathCos => Some(Self::eval_math_cos(args)),
            NativeFunctionKind::MathCosh => Some(Self::eval_math_cosh(args)),
            NativeFunctionKind::MathExp => Some(Self::eval_math_exp(args)),
            NativeFunctionKind::MathExpm1 => Some(Self::eval_math_expm1(args)),
            NativeFunctionKind::MathFloor => Some(Self::eval_math_floor(args)),
            NativeFunctionKind::MathFround => Some(Self::eval_math_fround(args)),
            NativeFunctionKind::MathHypot => Some(Self::eval_math_hypot(args)),
            NativeFunctionKind::MathImul => Some(Self::eval_math_imul(args)),
            NativeFunctionKind::MathLog => Some(Self::eval_math_log(args)),
            NativeFunctionKind::MathLog10 => Some(Self::eval_math_log10(args)),
            NativeFunctionKind::MathLog1p => Some(Self::eval_math_log1p(args)),
            NativeFunctionKind::MathLog2 => Some(Self::eval_math_log2(args)),
            NativeFunctionKind::MathMax => Some(Self::eval_math_max(args)),
            NativeFunctionKind::MathMin => Some(Self::eval_math_min(args)),
            NativeFunctionKind::MathPow => Some(Self::eval_math_pow(args)),
            NativeFunctionKind::MathRandom => Some(self.eval_math_random(args)),
            NativeFunctionKind::MathRound => Some(Self::eval_math_round(args)),
            NativeFunctionKind::MathSign => Some(Self::eval_math_sign(args)),
            NativeFunctionKind::MathSin => Some(Self::eval_math_sin(args)),
            NativeFunctionKind::MathSinh => Some(Self::eval_math_sinh(args)),
            NativeFunctionKind::MathSqrt => Some(Self::eval_math_sqrt(args)),
            NativeFunctionKind::MathTan => Some(Self::eval_math_tan(args)),
            NativeFunctionKind::MathTanh => Some(Self::eval_math_tanh(args)),
            NativeFunctionKind::MathTrunc => Some(Self::eval_math_trunc(args)),
            _ => None,
        }
    }
}
