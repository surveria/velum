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

mod regexp;
mod string;

const fn runtime_call_args(args: &[Value]) -> RuntimeCallArgs<'_> {
    RuntimeCallArgs::values(args)
}

impl Context {
    pub(crate) fn eval_direct_native_property_call(
        &mut self,
        target: NativeCallTarget,
        access: StaticPropertyAccessId,
        callee: &Value,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if !self.optional_optimizations_enabled() {
            return self.call_value(callee, args, this_value.clone());
        }
        if let Value::NativeFunction(id) = callee {
            if let Some(kind) = self.cached_static_property_native_call_kind(access, *id)? {
                self.record_native_call_cache_hit();
                return self.eval_direct_or_generic_native_function_kind(kind, args, this_value);
            }
            if let Some(kind) = self.direct_native_call_kind(*id, target) {
                self.record_native_call_cache_miss();
                self.remember_static_property_native_call_kind(access, *id, kind)?;
                return self.eval_direct_native_call_target(target, args, this_value);
            }
            let kind = self.native_function(*id)?.kind();
            if kind.to_call_target().is_some() {
                self.record_native_call_cache_miss();
                self.remember_static_property_native_call_kind(access, *id, kind)?;
                return self.eval_direct_or_generic_native_function_kind(kind, args, this_value);
            }
        }
        self.record_native_call_cache_slow_path();
        self.call_value(callee, args, this_value.clone())
    }

    pub(crate) fn eval_cached_direct_native_static_member_call(
        &mut self,
        target: NativeCallTarget,
        property: &StaticName,
        access: StaticPropertyAccessId,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Option<Value>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        let Value::Object(object) = this_value else {
            return Ok(None);
        };
        self.ensure_object_prototype_intrinsic_for_ordinary_lookup(*object, property.as_str())?;

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
                self.call_value(&Value::NativeFunction(function), args, this_value.clone())
                    .map(Some)
            }
            CacheableNativePropertyValue::Other(callee) => {
                self.record_native_call_cache_slow_path();
                self.call_value(&callee, args, this_value.clone()).map(Some)
            }
            CacheableNativePropertyValue::Missing => {
                self.record_native_call_cache_slow_path();
                self.call_value(&Value::Undefined, args, this_value.clone())
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
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        let Value::Object(object) = this_value else {
            return Ok(None);
        };
        if self.objects.array_len_if_array(*object)?.is_some() {
            return Ok(None);
        }
        self.ensure_object_prototype_intrinsic_for_ordinary_lookup(*object, property.name())?;
        let lookup = if property.key().is_some() {
            property.lookup()
        } else {
            self.property_lookup(property.name())
        };

        if let Some(kind) =
            self.cached_static_object_property_native_call_kind(access, *object, lookup)?
        {
            self.record_native_call_cache_hit();
            return self
                .eval_direct_or_generic_native_function_kind(kind, args, this_value)
                .map(Some);
        }

        let candidate = self.objects.cacheable_property_lookup(*object, lookup)?;
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
                self.call_value(&callee, args, this_value.clone()).map(Some)
            }
            CacheableNativePropertyValue::Missing => {
                self.record_native_call_cache_slow_path();
                self.call_value(&Value::Undefined, args, this_value.clone())
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
        if let Some(value) = self.eval_direct_primitive_native_call_target(target, args, this_value)
        {
            return value;
        }
        if let Some(value) = self.eval_direct_array_native_call_target(target, args, this_value) {
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
        if let Some(value) = self.eval_direct_regexp_call_target(target, args, this_value) {
            return value;
        }
        match target {
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
            NativeCallTarget::Date => self.eval_direct_date_constructor(args),
            NativeCallTarget::JsonIsRawJson => self.eval_direct_json_is_raw_json(args),
            NativeCallTarget::JsonParse => self.eval_direct_json_parse(args),
            NativeCallTarget::JsonRawJson => self.eval_direct_json_raw_json(args),
            NativeCallTarget::JsonStringify => self.eval_direct_json_stringify(args),
            NativeCallTarget::Number => self.eval_direct_number_constructor(args),
            NativeCallTarget::Promise => {
                Err(Error::type_error("Promise constructor requires 'new'"))
            }
            NativeCallTarget::PromiseResolve => {
                self.eval_promise_resolve(runtime_call_args(args), this_value)
            }
            NativeCallTarget::PromiseReject => {
                self.eval_promise_reject(runtime_call_args(args), this_value)
            }
            NativeCallTarget::PromiseThen => self.eval_direct_promise_then(args, this_value),
            NativeCallTarget::PromiseCatch => self.eval_direct_promise_catch(args, this_value),
            NativeCallTarget::PromiseFinally => {
                self.eval_promise_finally(runtime_call_args(args), this_value)
            }
            NativeCallTarget::Symbol => self.eval_symbol_constructor(runtime_call_args(args)),
            target => self
                .eval_direct_string_native_call_target(target, args, this_value)
                .ok_or_else(|| Error::runtime("String native call target was not handled"))?,
        }
    }

    fn eval_direct_math_call_target(
        &mut self,
        target: NativeCallTarget,
        args: &[Value],
    ) -> Option<Result<Value>> {
        match target {
            NativeCallTarget::MathAbs => Some(self.eval_direct_math_abs(args)),
            NativeCallTarget::MathAcos => Some(self.eval_direct_math_acos(args)),
            NativeCallTarget::MathAcosh => Some(self.eval_direct_math_acosh(args)),
            NativeCallTarget::MathAsin => Some(self.eval_direct_math_asin(args)),
            NativeCallTarget::MathAsinh => Some(self.eval_direct_math_asinh(args)),
            NativeCallTarget::MathAtan => Some(self.eval_direct_math_atan(args)),
            NativeCallTarget::MathAtan2 => Some(self.eval_direct_math_atan2(args)),
            NativeCallTarget::MathAtanh => Some(self.eval_direct_math_atanh(args)),
            NativeCallTarget::MathCbrt => Some(self.eval_direct_math_cbrt(args)),
            NativeCallTarget::MathCeil => Some(self.eval_direct_math_ceil(args)),
            NativeCallTarget::MathClz32 => Some(self.eval_direct_math_clz32(args)),
            NativeCallTarget::MathCos => Some(self.eval_direct_math_cos(args)),
            NativeCallTarget::MathCosh => Some(self.eval_direct_math_cosh(args)),
            NativeCallTarget::MathExp => Some(self.eval_direct_math_exp(args)),
            NativeCallTarget::MathExpm1 => Some(self.eval_direct_math_expm1(args)),
            NativeCallTarget::MathF16round => Some(self.eval_direct_math_f16round(args)),
            NativeCallTarget::MathFloor => Some(self.eval_direct_math_floor(args)),
            NativeCallTarget::MathFround => Some(self.eval_direct_math_fround(args)),
            NativeCallTarget::MathHypot => Some(self.eval_direct_math_hypot(args)),
            NativeCallTarget::MathImul => Some(self.eval_direct_math_imul(args)),
            NativeCallTarget::MathLog => Some(self.eval_direct_math_log(args)),
            NativeCallTarget::MathLog10 => Some(self.eval_direct_math_log10(args)),
            NativeCallTarget::MathLog1p => Some(self.eval_direct_math_log1p(args)),
            NativeCallTarget::MathLog2 => Some(self.eval_direct_math_log2(args)),
            NativeCallTarget::MathMax => Some(self.eval_direct_math_max(args)),
            NativeCallTarget::MathMin => Some(self.eval_direct_math_min(args)),
            NativeCallTarget::MathPow => Some(self.eval_direct_math_pow(args)),
            NativeCallTarget::MathRandom => Some(self.eval_direct_math_random(args)),
            NativeCallTarget::MathRound => Some(self.eval_direct_math_round(args)),
            NativeCallTarget::MathSign => Some(self.eval_direct_math_sign(args)),
            NativeCallTarget::MathSin => Some(self.eval_direct_math_sin(args)),
            NativeCallTarget::MathSinh => Some(self.eval_direct_math_sinh(args)),
            NativeCallTarget::MathSqrt => Some(self.eval_direct_math_sqrt(args)),
            NativeCallTarget::MathSumPrecise => Some(self.eval_direct_math_sum_precise(args)),
            NativeCallTarget::MathTan => Some(self.eval_direct_math_tan(args)),
            NativeCallTarget::MathTanh => Some(self.eval_direct_math_tanh(args)),
            NativeCallTarget::MathTrunc => Some(self.eval_direct_math_trunc(args)),
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
            NativeCallTarget::ObjectFreeze => Some(self.eval_direct_object_freeze(args)),
            NativeCallTarget::ObjectGetOwnPropertyDescriptor => {
                Some(self.eval_object_get_own_property_descriptor(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectGetOwnPropertyDescriptors => {
                Some(self.eval_object_get_own_property_descriptors(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectGetOwnPropertyNames => {
                Some(self.eval_object_get_own_property_names(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectGetOwnPropertySymbols => {
                Some(self.eval_object_get_own_property_symbols(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectGetPrototypeOf => {
                Some(self.eval_object_get_prototype_of(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectHasOwn => {
                Some(self.eval_object_has_own(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectIs => Some(Ok(Self::eval_direct_object_is(args))),
            NativeCallTarget::ObjectIsExtensible => {
                Some(self.eval_object_is_extensible(runtime_call_args(args)))
            }
            NativeCallTarget::ObjectIsFrozen => Some(self.eval_direct_object_is_frozen(args)),
            NativeCallTarget::ObjectIsSealed => Some(self.eval_direct_object_is_sealed(args)),
            NativeCallTarget::ObjectKeys => Some(self.eval_object_keys(runtime_call_args(args))),
            NativeCallTarget::ObjectPreventExtensions => {
                Some(self.eval_direct_object_prevent_extensions(args))
            }
            NativeCallTarget::ObjectSetPrototypeOf => {
                Some(self.eval_direct_object_set_prototype_of(args))
            }
            NativeCallTarget::ObjectSeal => Some(self.eval_direct_object_seal(args)),
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
            NativeCallTarget::GlobalIsFinite => Some(self.eval_direct_global_is_finite(args)),
            NativeCallTarget::GlobalIsNan => Some(self.eval_direct_global_is_nan(args)),
            NativeCallTarget::GlobalParseFloat => Some(self.eval_direct_global_parse_float(args)),
            NativeCallTarget::GlobalParseInt => Some(self.eval_direct_global_parse_int(args)),
            NativeCallTarget::NumberIsFinite => Some(Ok(Self::eval_direct_number_is_finite(args))),
            NativeCallTarget::NumberIsInteger => {
                Some(Ok(Self::eval_direct_number_is_integer(args)))
            }
            NativeCallTarget::NumberIsNan => Some(Ok(Self::eval_direct_number_is_nan(args))),
            NativeCallTarget::NumberIsSafeInteger => {
                Some(Ok(Self::eval_direct_number_is_safe_integer(args)))
            }
            _ => None,
        }
    }

    pub(in crate::runtime) fn direct_native_call_kind(
        &self,
        id: NativeFunctionId,
        target: NativeCallTarget,
    ) -> Option<NativeFunctionKind> {
        if !self.optional_optimizations_enabled() {
            return None;
        }
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
        if self.optional_optimizations_enabled()
            && let Some(target) = kind.to_call_target()
        {
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
        if let Some(value) = self.eval_reflect_native_function_kind(kind, args, this_value) {
            return value;
        }
        if let Some(result) = Self::eval_shared_function_accessor_kind(kind, this_value) {
            return result;
        }
        if let NativeFunctionKind::Temporal(kind) = kind {
            return self.eval_temporal_native_function_kind(kind, args, this_value);
        }
        if let NativeFunctionKind::Intl(kind) = kind {
            return self.eval_intl_native_function_kind(kind, args, this_value);
        }

        self.eval_non_object_native_function_kind(kind, args, this_value)
    }

    fn eval_non_object_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if let Some(result) = self.eval_iterator_native_function_kind(kind, args, this_value) {
            return result;
        }
        if let Some(result) = self.eval_collection_native_function_kind(kind, args, this_value) {
            return result;
        }
        if let Some(result) = self.eval_array_native_function_kind(kind, args, this_value) {
            return result;
        }
        if let Some(result) = self.eval_regexp_native_function_kind(kind, args, this_value) {
            return result;
        }
        if let Some(result) = self.eval_generator_native_function_kind(kind, args, this_value) {
            return result;
        }
        if let Some(result) = self.eval_promise_native_function_kind(kind, args, this_value) {
            return result;
        }
        if let Some(result) = self.eval_buffer_memory_native_function_kind(kind, args, this_value) {
            return result;
        }
        if let Some(result) = self.eval_error_native_function_kind(kind, args, this_value) {
            return result;
        }
        match kind {
            NativeFunctionKind::AsyncFunction => self.eval_async_function_constructor(args),
            NativeFunctionKind::AsyncGeneratorFunction => {
                self.eval_async_generator_function_constructor(args)
            }
            NativeFunctionKind::Boolean => self.eval_boolean_constructor(args),
            NativeFunctionKind::BoundFunction(id) => self.eval_bound_function(id, args),
            NativeFunctionKind::ShadowRealm(kind) => self.eval_shadow_realm(kind, args, this_value),
            NativeFunctionKind::Eval => self.eval_eval_function(args),
            NativeFunctionKind::Function => self.eval_function_constructor(args),
            NativeFunctionKind::GeneratorFunction => self.eval_generator_function_constructor(args),
            NativeFunctionKind::FunctionPrototypeBind => {
                self.eval_function_prototype_bind(args, this_value)
            }
            NativeFunctionKind::FunctionPrototypeCall => {
                self.eval_function_prototype_call(args, this_value)
            }
            NativeFunctionKind::FunctionPrototypeApply => {
                self.eval_function_prototype_apply(args, this_value)
            }
            NativeFunctionKind::FunctionPrototypeHasInstance => {
                self.eval_function_prototype_has_instance(args, this_value)
            }
            NativeFunctionKind::FunctionPrototypeToString => {
                self.eval_function_prototype_to_string(args, this_value)
            }
            NativeFunctionKind::Date(kind) => {
                self.eval_date_native_function_kind(kind, args, this_value)
            }
            NativeFunctionKind::DataView(kind) => {
                self.eval_data_view_native_function_kind(kind, args, this_value)
            }
            NativeFunctionKind::DisposableStack(method) => {
                self.eval_disposable_stack_function(method, args, this_value)
            }
            NativeFunctionKind::AsyncDisposableStack(method) => {
                self.eval_async_disposable_stack_function(method, args, this_value)
            }
            NativeFunctionKind::TypedArrayIntrinsic => {
                Err(Error::type_error("%TypedArray% is an abstract constructor"))
            }
            NativeFunctionKind::TypedArrayPrototype(kind) => {
                self.eval_typed_array_native_function_kind(kind, args, this_value)
            }
            NativeFunctionKind::JsonIsRawJson => self.eval_json_is_raw_json(args),
            NativeFunctionKind::JsonParse => self.eval_json_parse(args),
            NativeFunctionKind::JsonRawJson => self.eval_json_raw_json(args),
            NativeFunctionKind::JsonStringify => self.eval_json_stringify(args),
            NativeFunctionKind::Number => self.eval_number_constructor(args),
            NativeFunctionKind::BigInt => self.eval_bigint_constructor(args),
            NativeFunctionKind::BigIntAsIntN => self.eval_bigint_as_int_n(args),
            NativeFunctionKind::BigIntAsUintN => self.eval_bigint_as_uint_n(args),
            NativeFunctionKind::PerformanceNow => Ok(self.eval_performance_now()),
            NativeFunctionKind::Print => self.eval_print_call(args),
            NativeFunctionKind::Proxy => Self::eval_proxy_call(args),
            NativeFunctionKind::ProxyRevocable => self.eval_proxy_revocable(args),
            NativeFunctionKind::ProxyRevoke(id) => self.eval_proxy_revoke(id),
            NativeFunctionKind::Symbol => self.eval_symbol_constructor(args),
            NativeFunctionKind::SymbolFor => self.eval_symbol_for(args),
            NativeFunctionKind::SymbolKeyFor => self.eval_symbol_key_for(args),
            NativeFunctionKind::TypedArray(_) => {
                Err(Error::type_error("TypedArray constructor requires 'new'"))
            }
            kind => self
                .eval_primitive_native_function_kind(kind, args, this_value)
                .unwrap_or_else(|| {
                    self.eval_string_native_function_kind(kind, args, this_value)
                        .unwrap_or_else(|| {
                            Err(Error::runtime(
                                "String native function kind was not handled",
                            ))
                        })
                }),
        }
    }

    fn eval_shared_function_accessor_kind(
        kind: NativeFunctionKind,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::SpeciesGetter => Some(Ok(this_value.clone())),
            NativeFunctionKind::ThrowTypeError => Some(Err(Error::type_error(
                "restricted function property access",
            ))),
            _ => None,
        }
    }

    fn eval_generator_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        use crate::runtime::generator::GeneratorResumeKind;

        let (resume, asynchronous) = match kind {
            NativeFunctionKind::GeneratorNext => (GeneratorResumeKind::Next, false),
            NativeFunctionKind::GeneratorReturn => (GeneratorResumeKind::Return, false),
            NativeFunctionKind::GeneratorThrow => (GeneratorResumeKind::Throw, false),
            NativeFunctionKind::AsyncGeneratorNext => (GeneratorResumeKind::Next, true),
            NativeFunctionKind::AsyncGeneratorReturn => (GeneratorResumeKind::Return, true),
            NativeFunctionKind::AsyncGeneratorThrow => (GeneratorResumeKind::Throw, true),
            _ => return None,
        };
        Some(self.eval_generator_resume(args, this_value, resume, asynchronous))
    }

    fn eval_reflect_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::ReflectApply => Some(self.eval_reflect_apply(args, this_value)),
            NativeFunctionKind::ReflectConstruct => {
                Some(self.eval_reflect_construct(args, this_value))
            }
            NativeFunctionKind::ReflectDefineProperty => {
                Some(self.eval_reflect_define_property(args, this_value))
            }
            NativeFunctionKind::ReflectDeleteProperty => {
                Some(self.eval_reflect_delete_property(args, this_value))
            }
            NativeFunctionKind::ReflectGet => Some(self.eval_reflect_get(args, this_value)),
            NativeFunctionKind::ReflectGetOwnPropertyDescriptor => {
                Some(self.eval_reflect_get_own_property_descriptor(args, this_value))
            }
            NativeFunctionKind::ReflectGetPrototypeOf => {
                Some(self.eval_reflect_get_prototype_of(args, this_value))
            }
            NativeFunctionKind::ReflectHas => Some(self.eval_reflect_has(args, this_value)),
            NativeFunctionKind::ReflectIsExtensible => {
                Some(self.eval_reflect_is_extensible(args, this_value))
            }
            NativeFunctionKind::ReflectOwnKeys => {
                Some(self.eval_reflect_own_keys(args, this_value))
            }
            NativeFunctionKind::ReflectPreventExtensions => {
                Some(self.eval_reflect_prevent_extensions(args, this_value))
            }
            NativeFunctionKind::ReflectSet => Some(self.eval_reflect_set(args, this_value)),
            NativeFunctionKind::ReflectSetPrototypeOf => {
                Some(self.eval_reflect_set_prototype_of(args, this_value))
            }
            _ => None,
        }
    }

    fn eval_global_utility_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::AnnexBGlobal(kind) => Some(self.eval_annex_b_global(kind, args)),
            NativeFunctionKind::GlobalDecodeUri => Some(self.eval_global_decode_uri(args)),
            NativeFunctionKind::GlobalDecodeUriComponent => {
                Some(self.eval_global_decode_uri_component(args))
            }
            NativeFunctionKind::GlobalEncodeUri => Some(self.eval_global_encode_uri(args)),
            NativeFunctionKind::GlobalEncodeUriComponent => {
                Some(self.eval_global_encode_uri_component(args))
            }
            NativeFunctionKind::GlobalIsFinite => Some(self.eval_global_is_finite(args)),
            NativeFunctionKind::GlobalIsNan => Some(self.eval_global_is_nan(args)),
            NativeFunctionKind::GlobalParseFloat => Some(self.eval_global_parse_float(args)),
            NativeFunctionKind::GlobalParseInt => Some(self.eval_global_parse_int(args)),
            NativeFunctionKind::NumberIsFinite => Some(Ok(Self::eval_number_is_finite(args))),
            NativeFunctionKind::NumberIsInteger => Some(Ok(Self::eval_number_is_integer(args))),
            NativeFunctionKind::NumberIsNan => Some(Ok(Self::eval_number_is_nan(args))),
            NativeFunctionKind::NumberIsSafeInteger => {
                Some(Ok(Self::eval_number_is_safe_integer(args)))
            }
            _ => None,
        }
    }

    fn eval_math_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::MathAbs => Some(self.eval_math_abs(args)),
            NativeFunctionKind::MathAcos => Some(self.eval_math_acos(args)),
            NativeFunctionKind::MathAcosh => Some(self.eval_math_acosh(args)),
            NativeFunctionKind::MathAsin => Some(self.eval_math_asin(args)),
            NativeFunctionKind::MathAsinh => Some(self.eval_math_asinh(args)),
            NativeFunctionKind::MathAtan => Some(self.eval_math_atan(args)),
            NativeFunctionKind::MathAtan2 => Some(self.eval_math_atan2(args)),
            NativeFunctionKind::MathAtanh => Some(self.eval_math_atanh(args)),
            NativeFunctionKind::MathCbrt => Some(self.eval_math_cbrt(args)),
            NativeFunctionKind::MathCeil => Some(self.eval_math_ceil(args)),
            NativeFunctionKind::MathClz32 => Some(self.eval_math_clz32(args)),
            NativeFunctionKind::MathCos => Some(self.eval_math_cos(args)),
            NativeFunctionKind::MathCosh => Some(self.eval_math_cosh(args)),
            NativeFunctionKind::MathExp => Some(self.eval_math_exp(args)),
            NativeFunctionKind::MathExpm1 => Some(self.eval_math_expm1(args)),
            NativeFunctionKind::MathF16round => Some(self.eval_math_f16round(args)),
            NativeFunctionKind::MathFloor => Some(self.eval_math_floor(args)),
            NativeFunctionKind::MathFround => Some(self.eval_math_fround(args)),
            NativeFunctionKind::MathHypot => Some(self.eval_math_hypot(args)),
            NativeFunctionKind::MathImul => Some(self.eval_math_imul(args)),
            NativeFunctionKind::MathLog => Some(self.eval_math_log(args)),
            NativeFunctionKind::MathLog10 => Some(self.eval_math_log10(args)),
            NativeFunctionKind::MathLog1p => Some(self.eval_math_log1p(args)),
            NativeFunctionKind::MathLog2 => Some(self.eval_math_log2(args)),
            NativeFunctionKind::MathMax => Some(self.eval_math_max(args)),
            NativeFunctionKind::MathMin => Some(self.eval_math_min(args)),
            NativeFunctionKind::MathPow => Some(self.eval_math_pow(args)),
            NativeFunctionKind::MathRandom => Some(self.eval_math_random(args)),
            NativeFunctionKind::MathRound => Some(self.eval_math_round(args)),
            NativeFunctionKind::MathSign => Some(self.eval_math_sign(args)),
            NativeFunctionKind::MathSin => Some(self.eval_math_sin(args)),
            NativeFunctionKind::MathSinh => Some(self.eval_math_sinh(args)),
            NativeFunctionKind::MathSqrt => Some(self.eval_math_sqrt(args)),
            NativeFunctionKind::MathSumPrecise => Some(self.eval_math_sum_precise(args)),
            NativeFunctionKind::MathTan => Some(self.eval_math_tan(args)),
            NativeFunctionKind::MathTanh => Some(self.eval_math_tanh(args)),
            NativeFunctionKind::MathTrunc => Some(self.eval_math_trunc(args)),
            _ => None,
        }
    }
}
