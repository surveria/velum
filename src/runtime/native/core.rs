use crate::{
    api::native_call::NativeCallTarget,
    ast::{DeclKind, StaticPropertyAccessId},
    bytecode::BytecodeBinding,
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::BindingCell,
    runtime::call_args::RuntimeCallArgs,
    runtime::object::{ObjectPropertyInit, PropertyEnumerable},
    value::{ErrorName, ErrorObject, NativeFunctionId, Value},
};

use super::{
    ARRAY_NAME, BOOLEAN_NAME, INFINITY_NAME, JSON_NAME, MATH_NAME, NAN_NAME, NUMBER_NAME,
    NativeFunction, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, OBJECT_NAME, PROMISE_NAME,
    STRING_NAME,
};

impl Context {
    pub(crate) fn builtin_value(&mut self, name: &str) -> Result<Option<Value>> {
        match name {
            ARRAY_NAME => self.array_constructor_value().map(Some),
            BOOLEAN_NAME => self.boolean_constructor_value().map(Some),
            INFINITY_NAME => self
                .global_constant_value(INFINITY_NAME, Value::Number(f64::INFINITY))
                .map(Some),
            JSON_NAME => self.json_object_value().map(Some),
            MATH_NAME => self.math_object_value().map(Some),
            NAN_NAME => self
                .global_constant_value(NAN_NAME, Value::Number(f64::NAN))
                .map(Some),
            NUMBER_NAME => self.number_constructor_value().map(Some),
            OBJECT_NAME => self.object_constructor_value().map(Some),
            PROMISE_NAME => self.promise_constructor_value().map(Some),
            STRING_NAME => self.string_constructor_value().map(Some),
            _ => {
                let Some(name) =
                    ErrorName::from_constructor_name(name).filter(|name| name.is_standard())
                else {
                    return Ok(None);
                };
                self.error_constructor_value(name).map(Some)
            }
        }
    }

    pub(crate) fn constructor_binding_bytecode(
        &mut self,
        name: &BytecodeBinding,
    ) -> Result<Option<Value>> {
        if let Some(binding) = self.get_or_materialize_binding_bytecode(name)? {
            return Ok(Some(binding.value()));
        }
        Ok(None)
    }

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
        this_value: Value,
    ) -> Result<Value> {
        if let Value::NativeFunction(id) = callee {
            if let Some(kind) = self.cached_static_property_native_call_kind(access, id)? {
                self.record_native_call_cache_hit();
                return self.eval_direct_native_function_kind(target, kind, args, &this_value);
            }
            if let Some(kind) = self.direct_native_call_kind(id, target) {
                self.record_native_call_cache_miss();
                self.remember_static_property_native_call_kind(access, id, kind)?;
                return self.eval_direct_native_function_kind(target, kind, args, &this_value);
            }
        }
        self.record_native_call_cache_fallback();
        self.eval_call_value(callee, args, this_value)
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
            NativeFunctionKind::ErrorConstructor(name) => self.eval_error_constructor(name, args),
            NativeFunctionKind::JsonParse => self.eval_json_parse(args),
            NativeFunctionKind::JsonStringify => self.eval_json_stringify(args),
            NativeFunctionKind::MathAbs => self.eval_math_abs(args),
            NativeFunctionKind::MathAcos => self.eval_math_acos(args),
            NativeFunctionKind::MathAcosh => self.eval_math_acosh(args),
            NativeFunctionKind::MathAsin => self.eval_math_asin(args),
            NativeFunctionKind::MathAsinh => self.eval_math_asinh(args),
            NativeFunctionKind::MathAtan => self.eval_math_atan(args),
            NativeFunctionKind::MathAtan2 => self.eval_math_atan2(args),
            NativeFunctionKind::MathAtanh => self.eval_math_atanh(args),
            NativeFunctionKind::MathCbrt => self.eval_math_cbrt(args),
            NativeFunctionKind::MathCeil => self.eval_math_ceil(args),
            NativeFunctionKind::MathClz32 => self.eval_math_clz32(args),
            NativeFunctionKind::MathCos => self.eval_math_cos(args),
            NativeFunctionKind::MathCosh => self.eval_math_cosh(args),
            NativeFunctionKind::MathExp => self.eval_math_exp(args),
            NativeFunctionKind::MathExpm1 => self.eval_math_expm1(args),
            NativeFunctionKind::MathFloor => self.eval_math_floor(args),
            NativeFunctionKind::MathFround => self.eval_math_fround(args),
            NativeFunctionKind::MathHypot => self.eval_math_hypot(args),
            NativeFunctionKind::MathImul => self.eval_math_imul(args),
            NativeFunctionKind::MathLog => self.eval_math_log(args),
            NativeFunctionKind::MathLog10 => self.eval_math_log10(args),
            NativeFunctionKind::MathLog1p => self.eval_math_log1p(args),
            NativeFunctionKind::MathLog2 => self.eval_math_log2(args),
            NativeFunctionKind::MathMax => self.eval_math_max(args),
            NativeFunctionKind::MathMin => self.eval_math_min(args),
            NativeFunctionKind::MathPow => self.eval_math_pow(args),
            NativeFunctionKind::MathRandom => self.eval_math_random(args),
            NativeFunctionKind::MathRound => self.eval_math_round(args),
            NativeFunctionKind::MathSign => self.eval_math_sign(args),
            NativeFunctionKind::MathSin => self.eval_math_sin(args),
            NativeFunctionKind::MathSinh => self.eval_math_sinh(args),
            NativeFunctionKind::MathSqrt => self.eval_math_sqrt(args),
            NativeFunctionKind::MathTan => self.eval_math_tan(args),
            NativeFunctionKind::MathTanh => self.eval_math_tanh(args),
            NativeFunctionKind::MathTrunc => self.eval_math_trunc(args),
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

    pub(crate) fn construct_native_function(
        &mut self,
        id: NativeFunctionId,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.construct_native_function_kind(self.native_function(id)?.kind(), args)
    }

    pub(in crate::runtime) fn construct_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        match kind {
            NativeFunctionKind::Array => self.eval_array_constructor(args),
            NativeFunctionKind::ArrayConcat
            | NativeFunctionKind::ArrayIncludes
            | NativeFunctionKind::ArrayIndexOf
            | NativeFunctionKind::ArrayJoin
            | NativeFunctionKind::ArrayLastIndexOf
            | NativeFunctionKind::ArrayPop
            | NativeFunctionKind::ArrayPush
            | NativeFunctionKind::ArrayReverse
            | NativeFunctionKind::ArrayShift
            | NativeFunctionKind::ArraySlice
            | NativeFunctionKind::ArrayUnshift
            | NativeFunctionKind::JsonParse
            | NativeFunctionKind::JsonStringify
            | NativeFunctionKind::MathAbs
            | NativeFunctionKind::MathAcos
            | NativeFunctionKind::MathAcosh
            | NativeFunctionKind::MathAsin
            | NativeFunctionKind::MathAsinh
            | NativeFunctionKind::MathAtan
            | NativeFunctionKind::MathAtan2
            | NativeFunctionKind::MathAtanh
            | NativeFunctionKind::MathCbrt
            | NativeFunctionKind::MathCeil
            | NativeFunctionKind::MathClz32
            | NativeFunctionKind::MathCos
            | NativeFunctionKind::MathCosh
            | NativeFunctionKind::MathExp
            | NativeFunctionKind::MathExpm1
            | NativeFunctionKind::MathFloor
            | NativeFunctionKind::MathFround
            | NativeFunctionKind::MathHypot
            | NativeFunctionKind::MathImul
            | NativeFunctionKind::MathLog
            | NativeFunctionKind::MathLog10
            | NativeFunctionKind::MathLog1p
            | NativeFunctionKind::MathLog2
            | NativeFunctionKind::MathMax
            | NativeFunctionKind::MathMin
            | NativeFunctionKind::MathPow
            | NativeFunctionKind::MathRandom
            | NativeFunctionKind::MathRound
            | NativeFunctionKind::MathSign
            | NativeFunctionKind::MathSin
            | NativeFunctionKind::MathSinh
            | NativeFunctionKind::MathSqrt
            | NativeFunctionKind::MathTan
            | NativeFunctionKind::MathTanh
            | NativeFunctionKind::MathTrunc
            | NativeFunctionKind::ObjectDefineProperty
            | NativeFunctionKind::ObjectGetOwnPropertyDescriptor
            | NativeFunctionKind::ObjectHasOwn
            | NativeFunctionKind::ObjectKeys
            | NativeFunctionKind::PromiseResolve
            | NativeFunctionKind::PromiseReject
            | NativeFunctionKind::PromiseThen
            | NativeFunctionKind::PromiseCatch
            | NativeFunctionKind::PromiseResolver { .. } => {
                Err(Error::runtime("native method is not a constructor"))
            }
            NativeFunctionKind::Promise => self.eval_promise_constructor(args),
            NativeFunctionKind::Boolean => self.construct_boolean_object(args),
            NativeFunctionKind::ErrorConstructor(name) => self.eval_error_constructor(name, args),
            NativeFunctionKind::Number => self.construct_number_object(args),
            NativeFunctionKind::Object => self.eval_object_constructor(args),
            NativeFunctionKind::String => self.construct_string_object(args),
        }
    }

    pub(in crate::runtime) fn native_function(
        &self,
        id: NativeFunctionId,
    ) -> Result<&NativeFunction> {
        self.native_functions
            .get(id.index())
            .ok_or_else(|| Error::runtime("native function id is not defined"))
    }

    pub(in crate::runtime) fn native_function_mut(
        &mut self,
        id: NativeFunctionId,
    ) -> Result<&mut NativeFunction> {
        self.native_functions
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("native function id is not defined"))
    }

    fn error_constructor_value(&mut self, name: ErrorName) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::ErrorConstructor(name)) {
            return Ok(Value::NativeFunction(id));
        }

        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.error_prototype_with_constructor(constructor.clone())?;
        let function_kind = NativeFunctionKind::ErrorConstructor(name);
        let function_name = self.native_function_name_value(function_kind)?;
        self.push_native_function_with_id(id, function_kind, prototype, function_name)?;
        self.insert_global_builtin(name.as_str(), constructor.clone())?;
        Ok(constructor)
    }

    fn global_constant_value(&mut self, name: &str, value: Value) -> Result<Value> {
        self.insert_global_builtin(name, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::native) fn insert_global_builtin(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<()> {
        let atom = self.intern_atom(name)?;
        if self.builtin_globals.contains(atom) {
            return Ok(());
        }
        self.ensure_extra_binding_capacity(1)?;
        self.builtin_globals
            .insert(atom, BindingCell::new(value, false, DeclKind::Const));
        Ok(())
    }

    pub(in crate::runtime::native) fn object_prototype_id_with_constructor(
        &mut self,
        constructor: Value,
    ) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.define_non_enumerable_object_property(
            prototype,
            OBJECT_CONSTRUCTOR_PROPERTY,
            constructor,
        )?;
        Ok(Value::Object(prototype))
    }

    fn error_prototype_with_constructor(&mut self, constructor: Value) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects
            .create_with_prototype_property(
                None,
                ObjectPropertyInit::new(
                    constructor_key,
                    OBJECT_CONSTRUCTOR_PROPERTY,
                    constructor,
                    PropertyEnumerable::No,
                ),
                constructor_key,
                self.limits.max_objects,
                self.limits.max_object_properties,
            )
            .map(Value::Object)
    }

    pub(in crate::runtime::native) fn create_native_function(
        &mut self,
        kind: NativeFunctionKind,
        prototype: Value,
    ) -> Result<Value> {
        let name = self.native_function_name_value(kind)?;
        let id = self.next_native_function_id();
        self.push_native_function_with_id(id, kind, prototype, name)?;
        Ok(Value::NativeFunction(id))
    }

    pub(in crate::runtime) fn create_ephemeral_native_function(
        &mut self,
        kind: NativeFunctionKind,
        prototype: Value,
    ) -> Result<Value> {
        let name = self.native_function_name_value(kind)?;
        let id = self.next_native_function_id();
        self.push_native_function_unregistered_with_id(id, kind, prototype, name)?;
        Ok(Value::NativeFunction(id))
    }

    pub(in crate::runtime::native) const fn next_native_function_id(&self) -> NativeFunctionId {
        NativeFunctionId::new(self.native_functions.len())
    }

    pub(in crate::runtime::native) fn push_native_function_with_id(
        &mut self,
        id: NativeFunctionId,
        kind: NativeFunctionKind,
        prototype: Value,
        name: Value,
    ) -> Result<()> {
        if id.index() != self.native_functions.len() {
            return Err(Error::runtime(
                "native function id insertion order mismatch",
            ));
        }
        self.native_function_registry.insert(kind, id)?;
        self.push_native_function_unregistered_with_id(id, kind, prototype, name)
    }

    fn push_native_function_unregistered_with_id(
        &mut self,
        id: NativeFunctionId,
        kind: NativeFunctionKind,
        prototype: Value,
        name: Value,
    ) -> Result<()> {
        if id.index() != self.native_functions.len() {
            return Err(Error::runtime(
                "native function id insertion order mismatch",
            ));
        }
        self.native_functions
            .push(NativeFunction::new(kind, prototype, name));
        Ok(())
    }

    pub(in crate::runtime::native) fn native_function_name_value(
        &mut self,
        kind: NativeFunctionKind,
    ) -> Result<Value> {
        self.heap_string_value(kind.name())
    }

    pub(in crate::runtime::native) fn native_function_id(
        &self,
        kind: NativeFunctionKind,
    ) -> Option<NativeFunctionId> {
        self.native_function_registry.get(kind)
    }

    pub(super) const fn eval_native_unary_argument_value(
        args: RuntimeCallArgs<'_>,
    ) -> Option<&Value> {
        args.as_slice().first()
    }

    pub(in crate::runtime) fn eval_error_constructor(
        &self,
        name: ErrorName,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let message = args
            .as_slice()
            .first()
            .map_or_else(String::new, Value::display_for_concat);
        self.check_string_len(&message)?;
        Ok(Value::Error(ErrorObject::new(name, message)))
    }
}
