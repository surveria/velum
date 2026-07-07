use crate::{
    bytecode::BytecodeBinding,
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::BindingCell,
    runtime::call_args::RuntimeCallArgs,
    runtime::object::{ObjectPropertyInit, PropertyEnumerable},
    syntax::DeclKind,
    value::{ErrorName, ErrorObject, NativeFunctionId, Value},
};

use super::{
    ARRAY_NAME, BOOLEAN_NAME, EVAL_NAME, FUNCTION_NAME, INFINITY_NAME, JSON_NAME, MATH_NAME,
    NAN_NAME, NUMBER_NAME, NativeFunction, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY,
    OBJECT_NAME, PROMISE_NAME, REGEXP_NAME, STRING_NAME, SYMBOL_NAME,
};

impl Context {
    pub(crate) fn builtin_value(&mut self, name: &str) -> Result<Option<Value>> {
        match name {
            ARRAY_NAME => self.array_constructor_value().map(Some),
            BOOLEAN_NAME => self.boolean_constructor_value().map(Some),
            EVAL_NAME => self.eval_function_value().map(Some),
            FUNCTION_NAME => self.function_constructor_value().map(Some),
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
            REGEXP_NAME => self.regexp_constructor_value().map(Some),
            STRING_NAME => self.string_constructor_value().map(Some),
            SYMBOL_NAME => self.symbol_constructor_value().map(Some),
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
            NativeFunctionKind::AsyncFunction => self.eval_async_function_constructor(args),
            NativeFunctionKind::Function => self.eval_function_constructor(args),
            NativeFunctionKind::RegExp => self.construct_regexp_object(args),
            NativeFunctionKind::ArrayConcat
            | NativeFunctionKind::ArrayIncludes
            | NativeFunctionKind::ArrayIndexOf
            | NativeFunctionKind::ArrayIsArray
            | NativeFunctionKind::ArrayJoin
            | NativeFunctionKind::ArrayLastIndexOf
            | NativeFunctionKind::ArrayPop
            | NativeFunctionKind::ArrayPush
            | NativeFunctionKind::ArrayReverse
            | NativeFunctionKind::ArrayShift
            | NativeFunctionKind::ArraySlice
            | NativeFunctionKind::ArrayUnshift
            | NativeFunctionKind::BoundFunction(_)
            | NativeFunctionKind::Eval
            | NativeFunctionKind::FunctionPrototypeBind
            | NativeFunctionKind::FunctionPrototypeCall
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
            | NativeFunctionKind::ObjectGetPrototypeOf
            | NativeFunctionKind::ObjectGetOwnPropertyDescriptor
            | NativeFunctionKind::ObjectGetOwnPropertyNames
            | NativeFunctionKind::ObjectHasOwn
            | NativeFunctionKind::ObjectKeys
            | NativeFunctionKind::ObjectPrototypeHasOwnProperty
            | NativeFunctionKind::ObjectPrototypePropertyIsEnumerable
            | NativeFunctionKind::PromiseResolve
            | NativeFunctionKind::PromiseReject
            | NativeFunctionKind::PromiseThen
            | NativeFunctionKind::PromiseCatch
            | NativeFunctionKind::PromiseResolver { .. }
            | NativeFunctionKind::RegExpPrototypeTest
            | NativeFunctionKind::Symbol => {
                Err(Error::type_error("native method is not a constructor"))
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
        self.eval_direct_error_constructor(name, args.as_slice())
    }

    pub(in crate::runtime) fn eval_direct_error_constructor(
        &self,
        name: ErrorName,
        args: &[Value],
    ) -> Result<Value> {
        let message = args
            .first()
            .map_or_else(String::new, Value::display_for_concat);
        self.check_string_len(&message)?;
        Ok(Value::Error(ErrorObject::new(name, message)))
    }
}
