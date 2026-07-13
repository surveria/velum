use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        collections::{IteratorHelperMode, IteratorHelperState},
        native::IteratorFunctionKind,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
    },
    value::{ObjectId, Value},
};

use super::{
    ITERATOR_FROM_NAME, ITERATOR_PROTOTYPE_DROP_NAME, ITERATOR_PROTOTYPE_EVERY_NAME,
    ITERATOR_PROTOTYPE_FILTER_NAME, ITERATOR_PROTOTYPE_FIND_NAME, ITERATOR_PROTOTYPE_FLAT_MAP_NAME,
    ITERATOR_PROTOTYPE_FOR_EACH_NAME, ITERATOR_PROTOTYPE_MAP_NAME, ITERATOR_PROTOTYPE_REDUCE_NAME,
    ITERATOR_PROTOTYPE_SOME_NAME, ITERATOR_PROTOTYPE_TAKE_NAME, ITERATOR_PROTOTYPE_TO_ARRAY_NAME,
    NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY,
};

pub(in crate::runtime) const ITERATOR_GLOBAL_NAME: &str = "Iterator";
const ITERATOR_TAG: &str = "Iterator";
const ITERATOR_HELPER_TAG: &str = "Iterator Helper";
const ITERATOR_NEXT_NAME: &str = "next";
const ITERATOR_RETURN_NAME: &str = "return";
const PROTOTYPE_PROPERTY_NAME: &str = "prototype";
const ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.iterator]";
const ITERATOR_SYMBOL_DISPLAY_NAME: &str = "Symbol(Symbol.iterator)";
const TO_STRING_TAG_SYMBOL_DISPLAY: &str = "[Symbol.toStringTag]";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const DISPOSE_SYMBOL_DISPLAY: &str = "[Symbol.dispose]";
const DISPOSE_SYMBOL_PROPERTY: &str = "dispose";
const HIGH_SURROGATE_START: u16 = 0xD800;
const HIGH_SURROGATE_END: u16 = 0xDBFF;
const LOW_SURROGATE_START: u16 = 0xDC00;
const LOW_SURROGATE_END: u16 = 0xDFFF;

const ITERATOR_ABSTRACT_ERROR: &str =
    "Iterator is an abstract constructor and cannot be invoked directly";
const ITERATOR_RECEIVER_ERROR: &str = "Iterator helper method requires an object receiver";
const ITERATOR_CALLBACK_ERROR: &str = "Iterator helper callback must be callable";
const ITERATOR_LIMIT_NAN_ERROR: &str = "Iterator helper limit must not be NaN";
const ITERATOR_LIMIT_NEGATIVE_ERROR: &str = "Iterator helper limit must be non-negative";
const ITERATOR_FROM_PRIMITIVE_ERROR: &str = "Iterator.from source must be an object or a string";
const ITERATOR_FROM_RESULT_ERROR: &str = "Iterator.from source did not produce an object iterator";
const ITERATOR_PROTOTYPE_SETTER_ERROR: &str =
    "Iterator prototype accessor requires a distinct object receiver";

impl Context {
    pub(in crate::runtime::native) fn iterator_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = NativeFunctionKind::Iterator(IteratorFunctionKind::Constructor);
        if let Some(id) = self.native_function_id(constructor_kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let name = self.native_function_name_value(constructor_kind)?;
        self.push_native_function_with_id(id, constructor_kind, Value::Object(prototype), name)?;
        self.install_iterator_prototype_methods(prototype)?;
        let (helper_prototype, wrapped_prototype, collection_prototype) =
            self.create_iterator_intrinsic_prototypes(prototype)?;
        let from =
            self.iterator_method_value(NativeFunctionKind::Iterator(IteratorFunctionKind::From {
                helper_prototype,
                wrapped_prototype,
                collection_prototype,
            }))?;
        let from_key = self.intern_property_key(ITERATOR_FROM_NAME)?;
        self.define_native_function_property_key(
            id,
            ITERATOR_FROM_NAME,
            from_key,
            DataPropertyUpdate::new(
                Some(from),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )?;
        self.install_iterator_static_methods(id)?;
        self.insert_global_builtin(ITERATOR_GLOBAL_NAME, constructor.clone())?;
        Ok(constructor)
    }

    /// Returns the singleton native function for a slot-registered iterator
    /// kind, creating it on first use.
    fn iterator_method_value(&mut self, kind: NativeFunctionKind) -> Result<Value> {
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.create_native_function(kind, Value::Undefined)
    }

    fn install_iterator_prototype_methods(&mut self, prototype: ObjectId) -> Result<()> {
        self.install_iterator_constructor_accessor(prototype)?;
        let methods: &[(&str, IteratorFunctionKind)] = &[
            (
                ITERATOR_PROTOTYPE_MAP_NAME,
                IteratorFunctionKind::PrototypeMap,
            ),
            (
                ITERATOR_PROTOTYPE_FILTER_NAME,
                IteratorFunctionKind::PrototypeFilter,
            ),
            (
                ITERATOR_PROTOTYPE_TAKE_NAME,
                IteratorFunctionKind::PrototypeTake,
            ),
            (
                ITERATOR_PROTOTYPE_DROP_NAME,
                IteratorFunctionKind::PrototypeDrop,
            ),
            (
                ITERATOR_PROTOTYPE_FLAT_MAP_NAME,
                IteratorFunctionKind::PrototypeFlatMap,
            ),
            (
                ITERATOR_PROTOTYPE_REDUCE_NAME,
                IteratorFunctionKind::PrototypeReduce,
            ),
            (
                ITERATOR_PROTOTYPE_TO_ARRAY_NAME,
                IteratorFunctionKind::PrototypeToArray,
            ),
            (
                ITERATOR_PROTOTYPE_FOR_EACH_NAME,
                IteratorFunctionKind::PrototypeForEach,
            ),
            (
                ITERATOR_PROTOTYPE_SOME_NAME,
                IteratorFunctionKind::PrototypeSome,
            ),
            (
                ITERATOR_PROTOTYPE_EVERY_NAME,
                IteratorFunctionKind::PrototypeEvery,
            ),
            (
                ITERATOR_PROTOTYPE_FIND_NAME,
                IteratorFunctionKind::PrototypeFind,
            ),
        ];
        for (name, kind) in methods {
            let method = self.iterator_method_value(NativeFunctionKind::Iterator(*kind))?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        self.install_iterator_symbol_self(prototype)?;
        self.install_iterator_symbol_dispose(prototype)?;
        self.install_iterator_to_string_tag_accessor(prototype)
    }

    fn install_iterator_constructor_accessor(&mut self, prototype: ObjectId) -> Result<()> {
        let getter = self.iterator_method_value(NativeFunctionKind::Iterator(
            IteratorFunctionKind::PrototypeConstructorGetter,
        ))?;
        let setter = self.iterator_method_value(NativeFunctionKind::Iterator(
            IteratorFunctionKind::PrototypeConstructorSetter,
        ))?;
        let key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.define_property(
            prototype,
            key,
            OBJECT_CONSTRUCTOR_PROPERTY,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                Some(setter),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_iterator_symbol_self(&mut self, prototype: ObjectId) -> Result<()> {
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        let self_fn =
            self.create_native_function(NativeFunctionKind::IteratorSelf, Value::Undefined)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol),
            ITERATOR_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(self_fn),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        Ok(())
    }

    fn install_iterator_symbol_dispose(&mut self, prototype: ObjectId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let dispose_symbol = self.get_named(&symbol_constructor, DISPOSE_SYMBOL_PROPERTY)?;
        let Value::Symbol(symbol) = dispose_symbol else {
            return Err(Error::runtime("Symbol.dispose is not initialized"));
        };
        let dispose = self.iterator_method_value(NativeFunctionKind::Iterator(
            IteratorFunctionKind::PrototypeDispose,
        ))?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol.id()),
            DISPOSE_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(dispose),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_iterator_to_string_tag_accessor(&mut self, prototype: ObjectId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag_symbol = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(symbol) = tag_symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let getter = self.iterator_method_value(NativeFunctionKind::Iterator(
            IteratorFunctionKind::PrototypeToStringTagGetter,
        ))?;
        let setter = self.iterator_method_value(NativeFunctionKind::Iterator(
            IteratorFunctionKind::PrototypeToStringTagSetter,
        ))?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol.id()),
            TO_STRING_TAG_SYMBOL_DISPLAY,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                Some(setter),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_iterator_to_string_tag(&mut self, prototype: ObjectId, tag: &str) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag_symbol = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(symbol) = tag_symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let tag_value = self.heap_string_value(tag)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol.id()),
            TO_STRING_TAG_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(tag_value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        Ok(())
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_dispose(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        if let Some(return_method) = self.get_named_method(this_value, ITERATOR_RETURN_NAME)? {
            self.call_value(&return_method, &[], this_value.clone())?;
        }
        Ok(Value::Undefined)
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_getter(
        &mut self,
        kind: IteratorFunctionKind,
    ) -> Result<Value> {
        match kind {
            IteratorFunctionKind::PrototypeConstructorGetter => self.iterator_constructor_value(),
            IteratorFunctionKind::PrototypeToStringTagGetter => {
                self.heap_string_value(ITERATOR_TAG)
            }
            _ => Err(Error::runtime("invalid Iterator prototype getter kind")),
        }
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_setter(
        &mut self,
        kind: IteratorFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let property = self.iterator_prototype_setter_property(kind)?;
        let value = first_arg(&args);
        self.setter_that_ignores_iterator_prototype(property, this_value, value)?;
        Ok(Value::Undefined)
    }

    fn iterator_prototype_setter_property(
        &mut self,
        kind: IteratorFunctionKind,
    ) -> Result<DynamicPropertyKey> {
        match kind {
            IteratorFunctionKind::PrototypeConstructorSetter => Ok(DynamicPropertyKey::new(
                OBJECT_CONSTRUCTOR_PROPERTY.to_owned(),
                None,
            )),
            IteratorFunctionKind::PrototypeToStringTagSetter => {
                let symbol_constructor = self.symbol_constructor_value()?;
                let tag_symbol = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
                let Value::Symbol(symbol) = tag_symbol else {
                    return Err(Error::runtime("Symbol.toStringTag is not initialized"));
                };
                Ok(DynamicPropertyKey::new(
                    TO_STRING_TAG_SYMBOL_DISPLAY.to_owned(),
                    Some(PropertyKey::symbol(symbol.id())),
                ))
            }
            _ => Err(Error::runtime("invalid Iterator prototype setter kind")),
        }
    }

    /// Implements `SetterThatIgnoresPrototypeProperties` for the two special
    /// accessors installed on %Iterator.prototype%.
    fn setter_that_ignores_iterator_prototype(
        &mut self,
        mut property: DynamicPropertyKey,
        this_value: &Value,
        value: Value,
    ) -> Result<()> {
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(ITERATOR_PROTOTYPE_SETTER_ERROR));
        }
        let home = self.iterator_prototype_object_id()?;
        if this_value == &Value::Object(home) {
            return Err(Error::type_error(ITERATOR_PROTOTYPE_SETTER_ERROR));
        }
        if self
            .semantic_own_property_descriptor(this_value, &property)?
            .is_none()
        {
            let update = PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::Yes),
                Some(PropertyConfigurable::Yes),
            ));
            if self.semantic_define_own_property_update(this_value, &mut property, update)? {
                return Ok(());
            }
            return Err(Error::type_error(ITERATOR_PROTOTYPE_SETTER_ERROR));
        }
        if self.semantic_reflect_property_write(this_value, &mut property, value, this_value)?
            == Some(true)
        {
            return Ok(());
        }
        Err(Error::type_error(ITERATOR_PROTOTYPE_SETTER_ERROR))
    }

    /// %Iterator.prototype%, materializing the constructor on first use.
    pub(in crate::runtime) fn iterator_prototype_object_id(&mut self) -> Result<ObjectId> {
        let constructor = self.iterator_constructor_value()?;
        let prototype = self.get_named(&constructor, PROTOTYPE_PROPERTY_NAME)?;
        let Value::Object(id) = prototype else {
            return Err(Error::runtime("Iterator prototype is not an object"));
        };
        Ok(id)
    }

    fn create_iterator_intrinsic_prototypes(
        &mut self,
        parent: ObjectId,
    ) -> Result<(ObjectId, ObjectId, ObjectId)> {
        let constructor_key = self.object_constructor_property_key()?;
        let helper = self.objects.create_with_prototype(
            Some(parent),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(helper_prototype) = helper else {
            return Err(Error::runtime("iterator helper prototype creation failed"));
        };
        self.install_iterator_to_string_tag(helper_prototype, ITERATOR_HELPER_TAG)?;
        let wrapped = self.objects.create_with_prototype(
            Some(parent),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(wrapped_prototype) = wrapped else {
            return Err(Error::runtime("wrapped iterator prototype creation failed"));
        };
        let collection = self.objects.create_with_prototype(
            Some(parent),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(collection_prototype) = collection else {
            return Err(Error::runtime(
                "collection iterator prototype creation failed",
            ));
        };
        Ok((helper_prototype, wrapped_prototype, collection_prototype))
    }

    fn iterator_intrinsic_prototype_ids(&self) -> Result<(ObjectId, ObjectId, ObjectId)> {
        let placeholder = ObjectId::new(0);
        let lookup_kind = NativeFunctionKind::Iterator(IteratorFunctionKind::From {
            helper_prototype: placeholder,
            wrapped_prototype: placeholder,
            collection_prototype: placeholder,
        });
        let id = self
            .native_function_id(lookup_kind)
            .ok_or_else(|| Error::runtime("Iterator.from intrinsic is not initialized"))?;
        let NativeFunctionKind::Iterator(IteratorFunctionKind::From {
            helper_prototype,
            wrapped_prototype,
            collection_prototype,
        }) = self.native_function(id)?.kind()
        else {
            return Err(Error::runtime("Iterator.from intrinsic kind is invalid"));
        };
        Ok((helper_prototype, wrapped_prototype, collection_prototype))
    }

    /// %IteratorHelperPrototype%: shared parent of every lazy helper object.
    pub(in crate::runtime::native) fn iterator_helper_prototype_id(&self) -> Result<ObjectId> {
        self.iterator_intrinsic_prototype_ids()
            .map(|(helper, _wrapped, _collection)| helper)
    }

    /// %WrapForValidIteratorPrototype%: parent of `Iterator.from` wrappers.
    fn wrapped_iterator_prototype_id(&self) -> Result<ObjectId> {
        self.iterator_intrinsic_prototype_ids()
            .map(|(_helper, wrapped, _collection)| wrapped)
    }

    pub(in crate::runtime::native) fn collection_iterator_prototype_id(
        &mut self,
    ) -> Result<ObjectId> {
        self.iterator_constructor_value()?;
        self.iterator_intrinsic_prototype_ids()
            .map(|(_helper, _wrapped, collection)| collection)
    }

    pub(in crate::runtime::native) fn eval_iterator_abstract_call() -> Result<Value> {
        Err(Error::type_error(ITERATOR_ABSTRACT_ERROR))
    }

    /// `new.target`-aware construct used by `super()` in subclasses. Direct
    /// construction of the abstract class itself is rejected.
    pub(in crate::runtime) fn construct_iterator_object(
        &mut self,
        constructor: &Value,
        new_target: &Value,
    ) -> Result<Value> {
        if constructor == new_target {
            return Err(Error::type_error(ITERATOR_ABSTRACT_ERROR));
        }
        let prototype = Some(self.constructor_instance_prototype_with_default(
            new_target,
            NativeFunctionKind::Iterator(IteratorFunctionKind::Constructor),
        )?);
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype(
            prototype,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn require_iterator_receiver(&self, this_value: &Value) -> Result<()> {
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(ITERATOR_RECEIVER_ERROR));
        }
        Ok(())
    }

    fn require_callable_argument(&self, args: &RuntimeCallArgs<'_>) -> Result<Value> {
        let callback = first_arg(args);
        if !self.semantic_is_callable(&callback)? {
            return Err(Error::type_error(ITERATOR_CALLBACK_ERROR));
        }
        Ok(callback)
    }

    fn close_after_iterator_validation_error(&mut self, iterator: &Value, error: Error) -> Error {
        let mut source = crate::runtime::abstract_operations::IteratorSource::Protocol {
            iterator: iterator.clone(),
            next: Value::Undefined,
            done: false,
        };
        self.iterator_close_on_error(&mut source, error)
    }

    /// ECMAScript `GetIteratorDirect`: capture the receiver and its `next`.
    pub(in crate::runtime::native) fn iterator_direct_next(
        &mut self,
        iterator: &Value,
    ) -> Result<Value> {
        self.get_named(iterator, ITERATOR_NEXT_NAME)
    }

    fn create_iterator_helper_object(&mut self, state: IteratorHelperState) -> Result<Value> {
        let prototype = self.iterator_helper_prototype_id()?;
        let id = self.create_iterator_helper(state)?;
        let next = self.create_native_function(
            NativeFunctionKind::Iterator(IteratorFunctionKind::HelperNext(id)),
            Value::Undefined,
        )?;
        let return_fn = self.create_native_function(
            NativeFunctionKind::Iterator(IteratorFunctionKind::HelperReturn(id)),
            Value::Undefined,
        )?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("iterator helper object creation failed"));
        };
        self.define_non_enumerable_object_property(*object_id, ITERATOR_NEXT_NAME, next)?;
        self.define_non_enumerable_object_property(*object_id, ITERATOR_RETURN_NAME, return_fn)?;
        Ok(object)
    }

    fn create_callback_helper(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        mode: fn(Value) -> IteratorHelperMode,
    ) -> Result<Value> {
        self.require_iterator_receiver(this_value)?;
        let callback = match self.require_callable_argument(&args) {
            Ok(callback) => callback,
            Err(error) => {
                return Err(self.close_after_iterator_validation_error(this_value, error));
            }
        };
        let next = self.iterator_direct_next(this_value)?;
        self.create_iterator_helper_object(IteratorHelperState::new(
            this_value.clone(),
            next,
            mode(callback),
        ))
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_map(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.create_callback_helper(args, this_value, |mapper| IteratorHelperMode::Map {
            mapper,
        })
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_filter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.create_callback_helper(args, this_value, |predicate| IteratorHelperMode::Filter {
            predicate,
        })
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_flat_map(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.create_callback_helper(args, this_value, |mapper| IteratorHelperMode::FlatMap {
            mapper,
            inner: None,
        })
    }

    fn create_limit_helper(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        mode: fn(f64) -> IteratorHelperMode,
    ) -> Result<Value> {
        self.require_iterator_receiver(this_value)?;
        let integer = match self.validated_iterator_limit(&args) {
            Ok(integer) => integer,
            Err(error) => {
                return Err(self.close_after_iterator_validation_error(this_value, error));
            }
        };
        let next = self.iterator_direct_next(this_value)?;
        self.create_iterator_helper_object(IteratorHelperState::new(
            this_value.clone(),
            next,
            mode(integer),
        ))
    }

    fn validated_iterator_limit(&mut self, args: &RuntimeCallArgs<'_>) -> Result<f64> {
        let limit = first_arg(args);
        let number = self.to_number(&limit)?;
        if number.is_nan() {
            return Err(Error::exception(
                crate::value::ErrorName::RangeError,
                ITERATOR_LIMIT_NAN_ERROR,
            ));
        }
        let integer = self.to_integer_or_infinity(&Value::Number(number))?;
        if integer < 0.0 {
            return Err(Error::exception(
                crate::value::ErrorName::RangeError,
                ITERATOR_LIMIT_NEGATIVE_ERROR,
            ));
        }
        Ok(integer)
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_take(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.create_limit_helper(args, this_value, |remaining| IteratorHelperMode::Take {
            remaining,
        })
    }

    pub(in crate::runtime::native) fn eval_iterator_prototype_drop(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.create_limit_helper(args, this_value, |remaining| IteratorHelperMode::Drop {
            remaining,
        })
    }

    /// ECMAScript `Iterator.from` with `iterate-string-primitives` handling.
    pub(in crate::runtime::native) fn eval_iterator_from(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let input = first_arg(&args);
        let iterator = self.iterator_flattenable_source(&input)?;
        let prototype = self.iterator_prototype_object_id()?;
        if let Value::Object(id) = &iterator
            && self.objects.prototype_chain_has_object(*id, prototype)?
        {
            return Ok(iterator);
        }
        let next = self.iterator_direct_next(&iterator)?;
        let wrap_prototype = self.wrapped_iterator_prototype_id()?;
        let id = self.create_wrapped_iterator(iterator, next)?;
        let next_fn = self.create_native_function(
            NativeFunctionKind::Iterator(IteratorFunctionKind::WrapNext(id)),
            Value::Undefined,
        )?;
        let return_fn = self.create_native_function(
            NativeFunctionKind::Iterator(IteratorFunctionKind::WrapReturn(id)),
            Value::Undefined,
        )?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(wrap_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("wrapped iterator object creation failed"));
        };
        self.define_non_enumerable_object_property(*object_id, ITERATOR_NEXT_NAME, next_fn)?;
        self.define_non_enumerable_object_property(*object_id, ITERATOR_RETURN_NAME, return_fn)?;
        Ok(object)
    }

    /// ECMAScript `GetIteratorFlattenable` producing the iterator object.
    /// Strings are materialized through the snapshot iterator so a plain
    /// JavaScript iterator object always backs the result.
    fn iterator_flattenable_source(&mut self, input: &Value) -> Result<Value> {
        match input {
            Value::String(_) => self.iterator_string_source(input),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::BigInt(_)
            | Value::Symbol(_) => Err(Error::type_error(ITERATOR_FROM_PRIMITIVE_ERROR)),
            Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => self.iterator_object_source(input),
        }
    }

    fn iterator_object_source(&mut self, input: &Value) -> Result<Value> {
        let method = self.flattenable_iterator_method(input)?;
        let iterator = if let Some(method) = method {
            self.call_value(&method, &[], input.clone())?
        } else {
            input.clone()
        };
        if self.semantic_object_ref(&iterator)?.is_none() {
            return Err(Error::type_error(ITERATOR_FROM_RESULT_ERROR));
        }
        Ok(iterator)
    }

    fn iterator_string_source(&mut self, input: &Value) -> Result<Value> {
        if let Some(method) = self.flattenable_iterator_method(input)? {
            let iterator = self.call_value(&method, &[], input.clone())?;
            if self.semantic_object_ref(&iterator)?.is_none() {
                return Err(Error::type_error(ITERATOR_FROM_RESULT_ERROR));
            }
            return Ok(iterator);
        }
        let units = input
            .string_units()
            .ok_or_else(|| Error::runtime("string iterator source is not a string"))?;
        self.string_snapshot_iterator(&units)
    }

    fn string_snapshot_iterator(&mut self, units: &[u16]) -> Result<Value> {
        let mut items = Vec::with_capacity(units.len());
        let mut index = 0_usize;
        while let Some(first) = units.get(index).copied() {
            let paired = (HIGH_SURROGATE_START..=HIGH_SURROGATE_END).contains(&first)
                && units
                    .get(index.saturating_add(1))
                    .is_some_and(|next| (LOW_SURROGATE_START..=LOW_SURROGATE_END).contains(next));
            let width = if paired { 2_usize } else { 1_usize };
            let end = index
                .checked_add(width)
                .ok_or_else(|| Error::limit("string iterator index overflowed"))?;
            let code_point = units
                .get(index..end)
                .ok_or_else(|| Error::runtime("string iterator code point disappeared"))?;
            let value =
                if width == 1 && !(HIGH_SURROGATE_START..=LOW_SURROGATE_END).contains(&first) {
                    let ch = char::from_u32(u32::from(first))
                        .ok_or_else(|| Error::runtime("string iterator code point is invalid"))?;
                    self.heap_string_char_value(ch)?
                } else {
                    self.heap_utf16_string_value(code_point)?
                };
            items.push(value);
            index = end;
        }
        self.create_collection_iterator_object(items)
    }

    /// `GetMethod(input, @@iterator)` without touching the frozen abstract
    /// operation facade.
    pub(in crate::runtime::native) fn flattenable_iterator_method(
        &mut self,
        input: &Value,
    ) -> Result<Option<Value>> {
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Ok(None);
        };
        let key = DynamicPropertyKey::new(
            ITERATOR_SYMBOL_DISPLAY_NAME.to_owned(),
            Some(PropertyKey::symbol(symbol)),
        );
        self.get_method(input, key.lookup())
    }
}

fn first_arg(args: &RuntimeCallArgs<'_>) -> Value {
    args.as_slice().first().cloned().unwrap_or(Value::Undefined)
}
