use crate::{
    bytecode::BytecodeBinding,
    error::{Error, JavaScriptErrorMetadata, Result},
    runtime::binding::scope::BindingCell,
    runtime::call::RuntimeCallArgs,
    runtime::object::{ObjectPropertyInit, PropertyEnumerable, TypedArrayElementKind},
    runtime::{Context, abstract_operations::IteratorStep, roots::VmRootKind},
    syntax::DeclKind,
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

use super::{
    ARRAY_BUFFER_NAME, ARRAY_NAME, ASYNC_DISPOSABLE_STACK_NAME, ATOMICS_NAME,
    AsyncDisposableStackFunctionKind, BIGINT_NAME, BOOLEAN_NAME, DATA_VIEW_NAME, DATE_NAME,
    DISPOSABLE_STACK_NAME, DataViewFunctionKind, DateFunctionKind, DisposableStackFunctionKind,
    EVAL_NAME, FUNCTION_NAME, GLOBAL_DECODE_URI_COMPONENT_NAME, GLOBAL_DECODE_URI_NAME,
    GLOBAL_ENCODE_URI_COMPONENT_NAME, GLOBAL_ENCODE_URI_NAME, GLOBAL_IS_FINITE_NAME,
    GLOBAL_IS_NAN_NAME, GLOBAL_PARSE_FLOAT_NAME, GLOBAL_PARSE_INT_NAME, GLOBAL_THIS_NAME,
    INFINITY_NAME, ITERATOR_NAME, JSON_NAME, MAP_NAME, MATH_NAME, NAN_NAME, NUMBER_NAME,
    NativeFunction, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, OBJECT_NAME, PERFORMANCE_NAME,
    PROMISE_NAME, PROXY_NAME, REFLECT_NAME, REGEXP_NAME, SET_NAME, SHARED_ARRAY_BUFFER_NAME,
    STRING_NAME, SYMBOL_NAME, WEAK_MAP_NAME, WEAK_SET_NAME,
};

const NATIVE_METHOD_NOT_CONSTRUCTOR_ERROR: &str = "native method is not a constructor";
const ERROR_MESSAGE_PROPERTY: &str = "message";
const ERROR_NAME_PROPERTY: &str = "name";
const ERROR_ERRORS_PROPERTY: &str = "errors";
const ERROR_CAUSE_PROPERTY: &str = "cause";
const ERROR_PROTOTYPE_TO_STRING_NAME: &str = "toString";
const PRINT_NAME: &str = "print";

impl Context {
    pub(crate) fn builtin_value(&mut self, name: &str) -> Result<Option<Value>> {
        if let Some(element_kind) = typed_array_element_kind(name) {
            return self.typed_array_constructor_value(element_kind).map(Some);
        }
        match name {
            ATOMICS_NAME => self.atomics_object_value().map(Some),
            ARRAY_NAME => self.array_constructor_value().map(Some),
            ARRAY_BUFFER_NAME => self.array_buffer_constructor_value().map(Some),
            SHARED_ARRAY_BUFFER_NAME => self.shared_array_buffer_constructor_value().map(Some),
            DATA_VIEW_NAME => self.data_view_constructor_value().map(Some),
            BOOLEAN_NAME => self.boolean_constructor_value().map(Some),
            BIGINT_NAME => self.bigint_constructor_value().map(Some),
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
            ITERATOR_NAME => self.iterator_constructor_value().map(Some),
            JSON_NAME => self.json_object_value().map(Some),
            DATE_NAME => self.date_constructor_value().map(Some),
            ASYNC_DISPOSABLE_STACK_NAME => {
                self.async_disposable_stack_constructor_value().map(Some)
            }
            DISPOSABLE_STACK_NAME => self.disposable_stack_constructor_value().map(Some),
            MAP_NAME => self.map_constructor_value().map(Some),
            MATH_NAME => self.math_object_value().map(Some),
            NAN_NAME => self
                .global_constant_value(NAN_NAME, Value::Number(f64::NAN))
                .map(Some),
            NUMBER_NAME => self.number_constructor_value().map(Some),
            OBJECT_NAME => self.object_constructor_value().map(Some),
            PERFORMANCE_NAME => self.performance_object_value().map(Some),
            PRINT_NAME => self
                .global_function_value(NativeFunctionKind::Print)
                .map(Some),
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
        if let Some(element_kind) = typed_array_element_kind(name) {
            return self.typed_array_constructor_value(element_kind).map(Some);
        }
        match name {
            ARRAY_NAME => self.array_constructor_value().map(Some),
            ARRAY_BUFFER_NAME => self.array_buffer_constructor_value().map(Some),
            SHARED_ARRAY_BUFFER_NAME => self.shared_array_buffer_constructor_value().map(Some),
            DATA_VIEW_NAME => self.data_view_constructor_value().map(Some),
            BOOLEAN_NAME => self.boolean_constructor_value().map(Some),
            BIGINT_NAME => self.bigint_constructor_value().map(Some),
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
            ASYNC_DISPOSABLE_STACK_NAME => {
                self.async_disposable_stack_constructor_value().map(Some)
            }
            DISPOSABLE_STACK_NAME => self.disposable_stack_constructor_value().map(Some),
            ITERATOR_NAME => self.iterator_constructor_value().map(Some),
            MAP_NAME => self.map_constructor_value().map(Some),
            NUMBER_NAME => self.number_constructor_value().map(Some),
            OBJECT_NAME => self.object_constructor_value().map(Some),
            PRINT_NAME => self
                .global_function_value(NativeFunctionKind::Print)
                .map(Some),
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

    pub(in crate::runtime) fn construct_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        if !kind.is_constructable() {
            return Err(Error::type_error(NATIVE_METHOD_NOT_CONSTRUCTOR_ERROR));
        }
        match kind {
            NativeFunctionKind::Array => self.eval_array_constructor(args),
            NativeFunctionKind::ArrayBuffer => self.construct_array_buffer(args),
            NativeFunctionKind::SharedArrayBuffer => self.construct_shared_array_buffer(args),
            NativeFunctionKind::DataView(DataViewFunctionKind::Constructor) => {
                self.construct_data_view(args)
            }
            // Direct `new Iterator()` reaches this newTarget-less path only
            // when the constructor itself is the target, which the abstract
            // class rejects. Subclass construction flows through
            // `semantic_construct`.
            NativeFunctionKind::Iterator(_) => Self::eval_iterator_abstract_call(),
            NativeFunctionKind::AsyncFunction => self.eval_async_function_constructor(args),
            NativeFunctionKind::AsyncGeneratorFunction => {
                self.eval_async_generator_function_constructor(args)
            }
            NativeFunctionKind::Function => self.eval_function_constructor(args),
            NativeFunctionKind::GeneratorFunction => self.eval_generator_function_constructor(args),
            NativeFunctionKind::RegExp => self.construct_regexp_object(args),
            NativeFunctionKind::Promise => self.eval_promise_constructor(args),
            NativeFunctionKind::Boolean => self.construct_boolean_object(args),
            NativeFunctionKind::BigInt => Err(Error::type_error("BigInt is not a constructor")),
            NativeFunctionKind::ErrorConstructor(name) => self.eval_error_constructor(name, args),
            NativeFunctionKind::Number => self.construct_number_object(args),
            NativeFunctionKind::Object => self.eval_object_constructor(args),
            NativeFunctionKind::Proxy => self.construct_proxy_object(args),
            NativeFunctionKind::String => self.construct_string_object(args),
            NativeFunctionKind::Date(DateFunctionKind::Constructor) => {
                self.construct_date_object(args)
            }
            NativeFunctionKind::AsyncDisposableStack(
                AsyncDisposableStackFunctionKind::Constructor,
            ) => self.construct_async_disposable_stack(),
            NativeFunctionKind::DisposableStack(DisposableStackFunctionKind::Constructor) => {
                self.construct_disposable_stack()
            }
            NativeFunctionKind::TypedArray(element_kind) => {
                self.construct_typed_array(element_kind, args)
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

    pub(in crate::runtime) fn insert_global_builtin(
        &mut self,
        name: &str,
        value: Value,
    ) -> Result<()> {
        let atom = self.intern_atom(name)?;
        if self.realm.builtin_globals.contains(atom) {
            return Ok(());
        }
        self.ensure_extra_binding_capacity(1)?;
        self.realm
            .builtin_globals
            .insert(atom, BindingCell::new(value, false, DeclKind::Const))?;
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

    fn error_prototype_parent(&mut self, name: ErrorName) -> Result<Option<ObjectId>> {
        if matches!(name, ErrorName::Base) {
            self.object_constructor_value()?;
            let constructor_key = self.object_constructor_property_key()?;
            return self
                .objects
                .object_prototype_id(
                    constructor_key,
                    self.limits.max_objects,
                    self.limits.max_object_properties,
                )
                .map(Some);
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

    pub(in crate::runtime) fn create_native_function(
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

    pub(in crate::runtime) fn next_native_function_id(&self) -> NativeFunctionId {
        NativeFunctionId::new(self.native_functions.next_index())
    }

    pub(in crate::runtime) fn push_native_function_with_id(
        &mut self,
        id: NativeFunctionId,
        kind: NativeFunctionKind,
        prototype: Value,
        name: Value,
    ) -> Result<()> {
        if id.index() != self.native_functions.next_index() {
            return Err(Error::runtime(
                "native function id insertion order mismatch",
            ));
        }
        let adds_registry_entry = self
            .realm
            .native_function_registry
            .insertion_adds_entry(kind, id)?;
        let cache_reservation = if adds_registry_entry {
            Some(
                self.storage_ledger
                    .reserve_count(crate::runtime::VmStorageKind::CacheEntry, 1)?,
            )
        } else {
            None
        };
        self.realm.native_function_registry.insert(kind, id)?;
        if let Some(reservation) = cache_reservation {
            reservation.commit()?;
        }
        if let Err(error) =
            self.push_native_function_unregistered_with_id(id, kind, prototype, name)
        {
            if adds_registry_entry {
                self.realm.native_function_registry.remove(kind, id)?;
                self.storage_ledger
                    .release_count(crate::runtime::VmStorageKind::CacheEntry, 1)?;
            }
            return Err(error);
        }
        Ok(())
    }

    fn push_native_function_unregistered_with_id(
        &mut self,
        id: NativeFunctionId,
        kind: NativeFunctionKind,
        prototype: Value,
        name: Value,
    ) -> Result<()> {
        if id.index() != self.native_functions.next_index() {
            return Err(Error::runtime(
                "native function id insertion order mismatch",
            ));
        }
        self.native_functions.reserve_insert()?;
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::NativeFunction, 1)?;
        let mut function = NativeFunction::new(kind, prototype, name, self.active_realm_index());
        function
            .properties_mut()
            .activate_storage(self.storage_ledger.clone())?;
        reservation.commit()?;
        self.native_functions.insert_at_next(id.index(), function)?;
        Ok(())
    }

    pub(in crate::runtime) fn native_function_name_value(
        &mut self,
        kind: NativeFunctionKind,
    ) -> Result<Value> {
        self.heap_string_value(kind.name())
    }

    pub(in crate::runtime) fn native_function_id(
        &self,
        kind: NativeFunctionKind,
    ) -> Option<NativeFunctionId> {
        self.realm.native_function_registry.get(kind)
    }

    pub(super) const fn eval_native_unary_argument_value(
        args: RuntimeCallArgs<'_>,
    ) -> Option<&Value> {
        args.as_slice().first()
    }

    pub(in crate::runtime) fn eval_error_constructor(
        &mut self,
        name: ErrorName,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_error_constructor(name, args.as_slice())
    }

    pub(in crate::runtime) fn eval_direct_error_constructor(
        &mut self,
        name: ErrorName,
        args: &[Value],
    ) -> Result<Value> {
        self.eval_direct_error_constructor_with_prototype(name, args, None)
    }

    pub(in crate::runtime) fn eval_direct_error_constructor_with_prototype(
        &mut self,
        name: ErrorName,
        args: &[Value],
        prototype: Option<ObjectId>,
    ) -> Result<Value> {
        let message_value = Self::error_constructor_message_argument(name, args);
        let (message, define_message) = match message_value {
            None | Some(Value::Undefined) => (String::new(), false),
            Some(value) => (self.to_string(value)?, true),
        };
        let error = self.create_error_object_with_prototype(
            JavaScriptErrorMetadata::new(name, message),
            define_message,
            prototype,
        )?;
        if matches!(name, ErrorName::AggregateError) {
            let errors = args.first().cloned().unwrap_or(Value::Undefined);
            let errors = self.aggregate_error_list(&errors)?;
            self.define_aggregate_errors(&error, errors)?;
        }
        if matches!(name, ErrorName::SuppressedError) {
            let error_value = args.first().cloned().unwrap_or(Value::Undefined);
            let suppressed = args.get(1).cloned().unwrap_or(Value::Undefined);
            self.define_suppressed_error_fields(&error, error_value, suppressed)?;
        }
        if let Some(options) = Self::error_constructor_options_argument(name, args)
            && self.semantic_object_ref(options)?.is_some()
            && self.has_property_value_with_lookup(
                options,
                self.property_lookup(ERROR_CAUSE_PROPERTY),
            )?
        {
            let cause = self.get_named(options, ERROR_CAUSE_PROPERTY)?;
            let Value::Object(error) = error else {
                return Err(Error::runtime("Error allocation did not produce an object"));
            };
            self.define_non_enumerable_object_property(error, ERROR_CAUSE_PROPERTY, cause)?;
        }
        Ok(error)
    }

    pub(in crate::runtime) fn create_aggregate_error(&mut self, errors: Value) -> Result<Value> {
        let error = self.create_error_object(
            JavaScriptErrorMetadata::new(ErrorName::AggregateError, ""),
            false,
        )?;
        self.define_aggregate_errors(&error, errors)?;
        Ok(error)
    }

    fn aggregate_error_list(&mut self, errors: &Value) -> Result<Value> {
        let mut iterator = self.get_iterator(errors)?;
        let _iterator_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, iterator.root_values())?;
        let mut values = Vec::new();
        loop {
            self.step()?;
            match self.iterator_step(&mut iterator)? {
                IteratorStep::Value(value) => values.push(value),
                IteratorStep::Done => return self.create_array_from_elements(values),
                IteratorStep::Abrupt(completion) => return completion.into_result(),
            }
        }
    }

    fn define_aggregate_errors(&mut self, error: &Value, errors: Value) -> Result<()> {
        let Value::Object(error) = error else {
            return Err(Error::runtime(
                "AggregateError allocation did not produce an object",
            ));
        };
        self.define_non_enumerable_object_property(*error, ERROR_ERRORS_PROPERTY, errors)
    }

    pub(in crate::runtime) fn create_suppressed_error(
        &mut self,
        error: Value,
        suppressed: Value,
    ) -> Result<Value> {
        let value = self.create_error_object(
            JavaScriptErrorMetadata::new(ErrorName::SuppressedError, ""),
            false,
        )?;
        self.define_suppressed_error_fields(&value, error, suppressed)?;
        Ok(value)
    }

    fn define_suppressed_error_fields(
        &mut self,
        object: &Value,
        error: Value,
        suppressed: Value,
    ) -> Result<()> {
        let Value::Object(id) = object else {
            return Err(Error::runtime(
                "SuppressedError allocation did not produce an object",
            ));
        };
        self.define_non_enumerable_object_property(*id, "error", error)?;
        self.define_non_enumerable_object_property(*id, "suppressed", suppressed)
    }

    pub(in crate::runtime) fn create_error_object(
        &mut self,
        metadata: JavaScriptErrorMetadata,
        define_message: bool,
    ) -> Result<Value> {
        self.create_error_object_with_prototype(metadata, define_message, None)
    }

    fn create_error_object_with_prototype(
        &mut self,
        metadata: JavaScriptErrorMetadata,
        define_message: bool,
        prototype: Option<ObjectId>,
    ) -> Result<Value> {
        let prototype = if let Some(prototype) = prototype {
            prototype
        } else {
            self.error_constructor_prototype(metadata.error_name())?
        };
        let message = if define_message {
            Some(self.heap_string_value(metadata.message())?)
        } else {
            None
        };
        let Value::Object(id) = self
            .objects
            .create_with_exact_prototype(Some(prototype), self.limits.max_objects)?
        else {
            return Err(Error::runtime("Error allocation did not produce an object"));
        };
        self.objects.set_error_metadata(id, metadata)?;
        if let Some(message) = message {
            self.define_non_enumerable_object_property(id, ERROR_MESSAGE_PROPERTY, message)?;
        }
        Ok(Value::Object(id))
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
        match name {
            ErrorName::AggregateError => args.get(1),
            ErrorName::SuppressedError => args.get(2),
            _ => args.first(),
        }
    }

    fn error_constructor_options_argument(name: ErrorName, args: &[Value]) -> Option<&Value> {
        match name {
            ErrorName::AggregateError => args.get(2),
            ErrorName::SuppressedError => None,
            _ => args.get(1),
        }
    }

    fn error_to_string_property(
        &mut self,
        this_value: &Value,
        property: &str,
        default: &str,
    ) -> Result<String> {
        let value = self.get_named(this_value, property)?;
        if matches!(value, Value::Undefined) {
            return Ok(default.to_owned());
        }
        self.to_string(&value)
    }
}

fn typed_array_element_kind(name: &str) -> Option<TypedArrayElementKind> {
    TypedArrayElementKind::ALL
        .into_iter()
        .find(|element_kind| element_kind.name() == name)
}
