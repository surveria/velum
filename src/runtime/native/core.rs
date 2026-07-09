use crate::{
    bytecode::BytecodeBinding,
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::BindingCell,
    runtime::call::RuntimeCallArgs,
    runtime::object::{ObjectPropertyInit, PropertyEnumerable, PropertyLookup},
    syntax::DeclKind,
    value::{ErrorName, ErrorObject, NativeFunctionId, ObjectId, Value},
};

use super::{
    ARRAY_NAME, BOOLEAN_NAME, DATE_NAME, DateFunctionKind, EVAL_NAME, FUNCTION_NAME,
    GLOBAL_DECODE_URI_COMPONENT_NAME, GLOBAL_DECODE_URI_NAME, GLOBAL_ENCODE_URI_COMPONENT_NAME,
    GLOBAL_ENCODE_URI_NAME, GLOBAL_IS_FINITE_NAME, GLOBAL_IS_NAN_NAME, GLOBAL_PARSE_FLOAT_NAME,
    GLOBAL_PARSE_INT_NAME, GLOBAL_THIS_NAME, INFINITY_NAME, JSON_NAME, MAP_NAME, MATH_NAME,
    NAN_NAME, NUMBER_NAME, NativeFunction, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY,
    OBJECT_NAME, PROMISE_NAME, PROXY_NAME, REFLECT_NAME, REGEXP_NAME, SET_NAME, STRING_NAME,
    SYMBOL_NAME, WEAK_MAP_NAME, WEAK_SET_NAME,
};

const NATIVE_METHOD_NOT_CONSTRUCTOR_ERROR: &str = "native method is not a constructor";
const ERROR_MESSAGE_PROPERTY: &str = "message";
const ERROR_NAME_PROPERTY: &str = "name";
const ERROR_PROTOTYPE_TO_STRING_NAME: &str = "toString";

const fn native_kind_is_constructable(kind: NativeFunctionKind) -> bool {
    matches!(
        kind,
        NativeFunctionKind::Array
            | NativeFunctionKind::AsyncFunction
            | NativeFunctionKind::Boolean
            | NativeFunctionKind::ErrorConstructor(_)
            | NativeFunctionKind::Function
            | NativeFunctionKind::Number
            | NativeFunctionKind::Object
            | NativeFunctionKind::Promise
            | NativeFunctionKind::Proxy
            | NativeFunctionKind::RegExp
            | NativeFunctionKind::String
            | NativeFunctionKind::Map
            | NativeFunctionKind::Set
            | NativeFunctionKind::WeakMap
            | NativeFunctionKind::WeakSet
            | NativeFunctionKind::Date(DateFunctionKind::Constructor)
    )
}

impl Context {
    pub(crate) fn builtin_value(&mut self, name: &str) -> Result<Option<Value>> {
        match name {
            ARRAY_NAME => self.array_constructor_value().map(Some),
            BOOLEAN_NAME => self.boolean_constructor_value().map(Some),
            EVAL_NAME => self.eval_function_value().map(Some),
            FUNCTION_NAME => self.function_constructor_value().map(Some),
            GLOBAL_DECODE_URI_NAME => self
                .global_function_value(NativeFunctionKind::GlobalDecodeUri)
                .map(Some),
            GLOBAL_DECODE_URI_COMPONENT_NAME => self
                .global_function_value(NativeFunctionKind::GlobalDecodeUriComponent)
                .map(Some),
            GLOBAL_ENCODE_URI_NAME => self
                .global_function_value(NativeFunctionKind::GlobalEncodeUri)
                .map(Some),
            GLOBAL_ENCODE_URI_COMPONENT_NAME => self
                .global_function_value(NativeFunctionKind::GlobalEncodeUriComponent)
                .map(Some),
            GLOBAL_IS_FINITE_NAME => self
                .global_function_value(NativeFunctionKind::GlobalIsFinite)
                .map(Some),
            GLOBAL_IS_NAN_NAME => self
                .global_function_value(NativeFunctionKind::GlobalIsNan)
                .map(Some),
            GLOBAL_PARSE_FLOAT_NAME => self
                .global_function_value(NativeFunctionKind::GlobalParseFloat)
                .map(Some),
            GLOBAL_PARSE_INT_NAME => self
                .global_function_value(NativeFunctionKind::GlobalParseInt)
                .map(Some),
            GLOBAL_THIS_NAME => self.global_this_value().map(Some),
            INFINITY_NAME => self
                .global_constant_value(INFINITY_NAME, Value::Number(f64::INFINITY))
                .map(Some),
            JSON_NAME => self.json_object_value().map(Some),
            DATE_NAME => self.date_constructor_value().map(Some),
            MAP_NAME => self.map_constructor_value().map(Some),
            MATH_NAME => self.math_object_value().map(Some),
            NAN_NAME => self
                .global_constant_value(NAN_NAME, Value::Number(f64::NAN))
                .map(Some),
            NUMBER_NAME => self.number_constructor_value().map(Some),
            OBJECT_NAME => self.object_constructor_value().map(Some),
            PROMISE_NAME => self.promise_constructor_value().map(Some),
            PROXY_NAME => self.proxy_constructor_value().map(Some),
            REFLECT_NAME => self.reflect_object_value().map(Some),
            REGEXP_NAME => self.regexp_constructor_value().map(Some),
            SET_NAME => self.set_constructor_value().map(Some),
            STRING_NAME => self.string_constructor_value().map(Some),
            SYMBOL_NAME => self.symbol_constructor_value().map(Some),
            WEAK_MAP_NAME => self.weak_map_constructor_value().map(Some),
            WEAK_SET_NAME => self.weak_set_constructor_value().map(Some),
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

    pub(crate) fn direct_builtin_callable_value(&mut self, name: &str) -> Result<Option<Value>> {
        match name {
            ARRAY_NAME => self.array_constructor_value().map(Some),
            BOOLEAN_NAME => self.boolean_constructor_value().map(Some),
            EVAL_NAME => self.eval_function_value().map(Some),
            FUNCTION_NAME => self.function_constructor_value().map(Some),
            GLOBAL_DECODE_URI_NAME => self
                .global_function_value(NativeFunctionKind::GlobalDecodeUri)
                .map(Some),
            GLOBAL_DECODE_URI_COMPONENT_NAME => self
                .global_function_value(NativeFunctionKind::GlobalDecodeUriComponent)
                .map(Some),
            GLOBAL_ENCODE_URI_NAME => self
                .global_function_value(NativeFunctionKind::GlobalEncodeUri)
                .map(Some),
            GLOBAL_ENCODE_URI_COMPONENT_NAME => self
                .global_function_value(NativeFunctionKind::GlobalEncodeUriComponent)
                .map(Some),
            GLOBAL_IS_FINITE_NAME => self
                .global_function_value(NativeFunctionKind::GlobalIsFinite)
                .map(Some),
            GLOBAL_IS_NAN_NAME => self
                .global_function_value(NativeFunctionKind::GlobalIsNan)
                .map(Some),
            GLOBAL_PARSE_FLOAT_NAME => self
                .global_function_value(NativeFunctionKind::GlobalParseFloat)
                .map(Some),
            GLOBAL_PARSE_INT_NAME => self
                .global_function_value(NativeFunctionKind::GlobalParseInt)
                .map(Some),
            DATE_NAME => self.date_constructor_value().map(Some),
            MAP_NAME => self.map_constructor_value().map(Some),
            NUMBER_NAME => self.number_constructor_value().map(Some),
            OBJECT_NAME => self.object_constructor_value().map(Some),
            PROMISE_NAME => self.promise_constructor_value().map(Some),
            PROXY_NAME => self.proxy_constructor_value().map(Some),
            REGEXP_NAME => self.regexp_constructor_value().map(Some),
            SET_NAME => self.set_constructor_value().map(Some),
            STRING_NAME => self.string_constructor_value().map(Some),
            SYMBOL_NAME => self.symbol_constructor_value().map(Some),
            WEAK_MAP_NAME => self.weak_map_constructor_value().map(Some),
            WEAK_SET_NAME => self.weak_set_constructor_value().map(Some),
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
            return Ok(Some(binding.value(name.name())?));
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
        if !native_kind_is_constructable(kind) {
            return Err(Error::type_error(NATIVE_METHOD_NOT_CONSTRUCTOR_ERROR));
        }
        match kind {
            NativeFunctionKind::Array => self.eval_array_constructor(args),
            NativeFunctionKind::AsyncFunction => self.eval_async_function_constructor(args),
            NativeFunctionKind::Function => self.eval_function_constructor(args),
            NativeFunctionKind::RegExp => self.construct_regexp_object(args),
            NativeFunctionKind::Promise => self.eval_promise_constructor(args),
            NativeFunctionKind::Boolean => self.construct_boolean_object(args),
            NativeFunctionKind::ErrorConstructor(name) => self.eval_error_constructor(name, args),
            NativeFunctionKind::Number => self.construct_number_object(args),
            NativeFunctionKind::Object => self.eval_object_constructor(args),
            NativeFunctionKind::Proxy => self.construct_proxy_object(args),
            NativeFunctionKind::String => self.construct_string_object(args),
            NativeFunctionKind::Date(DateFunctionKind::Constructor) => {
                self.construct_date_object(args)
            }
            NativeFunctionKind::Map => self.construct_collection_object(
                crate::runtime::collections::CollectionKind::Map,
                args,
            ),
            NativeFunctionKind::Set => self.construct_collection_object(
                crate::runtime::collections::CollectionKind::Set,
                args,
            ),
            NativeFunctionKind::WeakMap => self.construct_weak_collection_object(
                crate::runtime::collections::CollectionKind::WeakMap,
                args,
            ),
            NativeFunctionKind::WeakSet => self.construct_weak_collection_object(
                crate::runtime::collections::CollectionKind::WeakSet,
                args,
            ),
            _ => Err(Error::type_error(NATIVE_METHOD_NOT_CONSTRUCTOR_ERROR)),
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

    pub(in crate::runtime) fn error_constructor_value(&mut self, name: ErrorName) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::ErrorConstructor(name)) {
            return Ok(Value::NativeFunction(id));
        }

        let prototype_parent = self.error_prototype_parent(name)?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype =
            self.error_prototype_with_constructor(name, constructor.clone(), prototype_parent)?;
        let Value::Object(prototype_id) = prototype else {
            return Err(Error::runtime("Error prototype is not an object"));
        };
        let function_kind = NativeFunctionKind::ErrorConstructor(name);
        let function_name = self.native_function_name_value(function_kind)?;
        self.push_native_function_with_id(
            id,
            function_kind,
            Value::Object(prototype_id),
            function_name,
        )?;
        if matches!(name, ErrorName::Base) {
            self.install_error_prototype_methods(prototype_id)?;
        }
        self.insert_global_builtin(name.as_str(), constructor.clone())?;
        Ok(constructor)
    }

    fn global_constant_value(&mut self, name: &str, value: Value) -> Result<Value> {
        self.insert_global_builtin(name, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::native) fn global_function_value(
        &mut self,
        kind: NativeFunctionKind,
    ) -> Result<Value> {
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        let function = self.create_native_function(kind, Value::Undefined)?;
        self.insert_global_builtin(kind.name(), function.clone())?;
        Ok(function)
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

    pub(in crate::runtime) fn error_constructor_prototype(
        &mut self,
        name: ErrorName,
    ) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.error_constructor_value(name)? else {
            return Err(Error::runtime("Error constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("Error prototype is not an object")),
        }
    }

    pub(in crate::runtime) fn error_prototype_property_value(
        &mut self,
        name: ErrorName,
        property: &str,
    ) -> Result<Value> {
        let prototype = self.error_constructor_prototype(name)?;
        self.get_property_value(&Value::Object(prototype), property)
    }

    pub(in crate::runtime) fn error_prototype_has_property(
        &mut self,
        name: ErrorName,
        property: PropertyLookup<'_>,
    ) -> Result<bool> {
        let prototype = self.error_constructor_prototype(name)?;
        self.objects.has(prototype, property)
    }

    fn error_prototype_parent(&mut self, name: ErrorName) -> Result<Option<ObjectId>> {
        if matches!(name, ErrorName::Base) {
            return Ok(None);
        }
        self.error_constructor_prototype(ErrorName::Base).map(Some)
    }

    fn error_prototype_with_constructor(
        &mut self,
        name: ErrorName,
        constructor: Value,
        prototype_parent: Option<ObjectId>,
    ) -> Result<Value> {
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_property(
            prototype_parent,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor,
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        self.install_error_prototype_properties(name, prototype)?;
        Ok(Value::Object(prototype))
    }

    fn install_error_prototype_properties(
        &mut self,
        name: ErrorName,
        prototype: ObjectId,
    ) -> Result<()> {
        let name_value = self.heap_string_value(name.as_str())?;
        self.define_non_enumerable_object_property(prototype, ERROR_NAME_PROPERTY, name_value)?;
        let message_value = self.heap_string_value("")?;
        self.define_non_enumerable_object_property(
            prototype,
            ERROR_MESSAGE_PROPERTY,
            message_value,
        )?;
        Ok(())
    }

    fn install_error_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        let to_string = self
            .create_native_function(NativeFunctionKind::ErrorPrototypeToString, Value::Undefined)?;
        self.define_non_enumerable_object_property(
            prototype,
            ERROR_PROTOTYPE_TO_STRING_NAME,
            to_string,
        )
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
        let message_value = Self::error_constructor_message_argument(name, args);
        let message = message_value.map_or_else(String::new, Value::display_for_concat);
        self.check_string_len(&message)?;
        Ok(Value::Error(ErrorObject::new(name, message)))
    }

    pub(in crate::runtime) fn eval_error_prototype_to_string(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_error_prototype_to_string(this_value)
    }

    pub(in crate::runtime) fn eval_direct_error_prototype_to_string(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "Error.prototype.toString receiver is nullish",
            ));
        }
        let name = self.error_to_string_property(this_value, ERROR_NAME_PROPERTY, "Error")?;
        let message = self.error_to_string_property(this_value, ERROR_MESSAGE_PROPERTY, "")?;
        let text = if name.is_empty() {
            message
        } else if message.is_empty() {
            name
        } else {
            let capacity = name
                .len()
                .checked_add(message.len())
                .and_then(|value| value.checked_add(2))
                .ok_or_else(|| Error::limit("string length exceeded supported range"))?;
            let mut text = String::with_capacity(capacity);
            text.push_str(&name);
            text.push_str(": ");
            text.push_str(&message);
            text
        };
        self.heap_string_owned_value(text)
    }

    fn error_constructor_message_argument(name: ErrorName, args: &[Value]) -> Option<&Value> {
        if matches!(name, ErrorName::AggregateError) {
            return args.get(1);
        }
        args.first()
    }

    fn error_to_string_property(
        &mut self,
        this_value: &Value,
        property: &str,
        default: &str,
    ) -> Result<String> {
        let value = self.get_property_value(this_value, property)?;
        let text = match value {
            Value::Undefined => default.to_owned(),
            Value::String(value) => value,
            Value::HeapString(value) => value.as_str().to_owned(),
            _ => value.display_for_concat(),
        };
        self.check_string_len(&text)?;
        Ok(text)
    }
}
