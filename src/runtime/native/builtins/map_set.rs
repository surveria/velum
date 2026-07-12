use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        collections::{CollectionIteratorId, CollectionKind},
        control::Completion,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, OwnPropertyDescriptor,
            PropertyConfigurable, PropertyEnumerable, PropertyKey, PropertyUpdate,
            PropertyWritable,
        },
        property::DynamicPropertyKey,
    },
    value::{ObjectId, Value},
};

use super::{NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY};

pub(in crate::runtime::native) const MAP_NAME: &str = "Map";
pub(in crate::runtime::native) const SET_NAME: &str = "Set";
const COLLECTION_GET_NAME: &str = "get";
const COLLECTION_SET_NAME: &str = "set";
const COLLECTION_ADD_NAME: &str = "add";
const COLLECTION_HAS_NAME: &str = "has";
const COLLECTION_DELETE_NAME: &str = "delete";
const COLLECTION_CLEAR_NAME: &str = "clear";
const COLLECTION_FOR_EACH_NAME: &str = "forEach";
const COLLECTION_ENTRIES_NAME: &str = "entries";
const COLLECTION_KEYS_NAME: &str = "keys";
const COLLECTION_VALUES_NAME: &str = "values";
const SET_UNION_NAME: &str = "union";
const SET_INTERSECTION_NAME: &str = "intersection";
const SET_DIFFERENCE_NAME: &str = "difference";
const SET_SYMMETRIC_DIFFERENCE_NAME: &str = "symmetricDifference";
const SET_IS_SUBSET_OF_NAME: &str = "isSubsetOf";
const SET_IS_SUPERSET_OF_NAME: &str = "isSupersetOf";
const SET_IS_DISJOINT_FROM_NAME: &str = "isDisjointFrom";
const COLLECTION_SIZE_NAME: &str = "size";
const ITERATOR_NEXT_NAME: &str = "next";
const ITERATOR_RESULT_VALUE_NAME: &str = "value";
const ITERATOR_RESULT_DONE_NAME: &str = "done";
const ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.iterator]";
const TO_STRING_TAG_SYMBOL_DISPLAY: &str = "[Symbol.toStringTag]";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const CONSTRUCTOR_REQUIRES_NEW_ERROR: &str = "constructor requires 'new'";
const MAP_ENTRY_NOT_OBJECT_ERROR: &str = "Map iterable entries must be objects";
const FOR_EACH_CALLBACK_ERROR: &str = "forEach callback must be callable";
const COLLECTION_ADDER_ERROR: &str = "collection adder must be callable";
const COLLECTION_ITERATOR_RECEIVER_ERROR: &str =
    "Collection Iterator.prototype.next requires a compatible iterator receiver";
const COLLECTION_ITERATOR_STATE_PROPERTY: &str = "\0CollectionIteratorState";
const MAP_ITERATOR_TAG: &str = "Map Iterator";
const SET_ITERATOR_TAG: &str = "Set Iterator";

pub(in crate::runtime) use crate::runtime::collections::CollectionIterationTarget;

impl Context {
    pub(in crate::runtime::native) fn map_constructor_value(&mut self) -> Result<Value> {
        self.collection_constructor_value(CollectionKind::Map)
    }

    pub(in crate::runtime::native) fn set_constructor_value(&mut self) -> Result<Value> {
        self.collection_constructor_value(CollectionKind::Set)
    }

    pub(in crate::runtime::native) fn collection_constructor_value(
        &mut self,
        kind: CollectionKind,
    ) -> Result<Value> {
        let constructor_kind = match kind {
            CollectionKind::Map => NativeFunctionKind::Map,
            CollectionKind::Set => NativeFunctionKind::Set,
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to Map or Set constructor",
                ));
            }
        };
        if let Some(id) = self.native_function_id(constructor_kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor.clone(),
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let name = self.native_function_name_value(constructor_kind)?;
        self.push_native_function_with_id(id, constructor_kind, Value::Object(prototype), name)?;
        self.install_species_accessor(id)?;
        self.install_collection_prototype_methods(prototype, kind)?;
        if kind == CollectionKind::Map {
            self.install_modern_map_constructor_methods(id)?;
        }
        let global_name = match kind {
            CollectionKind::Map => MAP_NAME,
            CollectionKind::Set => SET_NAME,
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to Map or Set global",
                ));
            }
        };
        self.insert_global_builtin(global_name, constructor.clone())?;
        Ok(constructor)
    }

    /// Returns the singleton native function for a slot-registered kind,
    /// creating it on first use so prototype aliases can share one function.
    fn collection_method_value(&mut self, kind: NativeFunctionKind) -> Result<Value> {
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.create_native_function(kind, Value::Undefined)
    }

    fn install_collection_prototype_methods(
        &mut self,
        prototype: crate::value::ObjectId,
        kind: CollectionKind,
    ) -> Result<()> {
        let methods: &[(&str, NativeFunctionKind)] = match kind {
            CollectionKind::Map => &[
                (COLLECTION_GET_NAME, NativeFunctionKind::MapGet),
                (COLLECTION_SET_NAME, NativeFunctionKind::MapSet),
                (COLLECTION_HAS_NAME, NativeFunctionKind::MapHas),
                (COLLECTION_DELETE_NAME, NativeFunctionKind::MapDelete),
                (COLLECTION_CLEAR_NAME, NativeFunctionKind::MapClear),
                (COLLECTION_FOR_EACH_NAME, NativeFunctionKind::MapForEach),
                (COLLECTION_ENTRIES_NAME, NativeFunctionKind::MapEntries),
                (COLLECTION_KEYS_NAME, NativeFunctionKind::MapKeys),
                (COLLECTION_VALUES_NAME, NativeFunctionKind::MapValues),
            ],
            CollectionKind::Set => &[
                (COLLECTION_ADD_NAME, NativeFunctionKind::SetAdd),
                (COLLECTION_HAS_NAME, NativeFunctionKind::SetHas),
                (COLLECTION_DELETE_NAME, NativeFunctionKind::SetDelete),
                (COLLECTION_CLEAR_NAME, NativeFunctionKind::SetClear),
                (COLLECTION_FOR_EACH_NAME, NativeFunctionKind::SetForEach),
                (COLLECTION_ENTRIES_NAME, NativeFunctionKind::SetEntries),
                (COLLECTION_KEYS_NAME, NativeFunctionKind::SetValues),
                (COLLECTION_VALUES_NAME, NativeFunctionKind::SetValues),
                (SET_UNION_NAME, NativeFunctionKind::SetUnion),
                (SET_INTERSECTION_NAME, NativeFunctionKind::SetIntersection),
                (SET_DIFFERENCE_NAME, NativeFunctionKind::SetDifference),
                (
                    SET_SYMMETRIC_DIFFERENCE_NAME,
                    NativeFunctionKind::SetSymmetricDifference,
                ),
                (SET_IS_SUBSET_OF_NAME, NativeFunctionKind::SetIsSubsetOf),
                (SET_IS_SUPERSET_OF_NAME, NativeFunctionKind::SetIsSupersetOf),
                (
                    SET_IS_DISJOINT_FROM_NAME,
                    NativeFunctionKind::SetIsDisjointFrom,
                ),
            ],
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to Map or Set prototype",
                ));
            }
        };
        for (name, method_kind) in methods {
            let method = self.collection_method_value(*method_kind)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        if kind == CollectionKind::Map {
            self.install_modern_map_prototype_methods(prototype)?;
        }
        let size_kind = match kind {
            CollectionKind::Map => NativeFunctionKind::MapSizeGetter,
            CollectionKind::Set => NativeFunctionKind::SetSizeGetter,
            _ => {
                return Err(Error::runtime("weak collection routed to Map or Set size"));
            }
        };
        let getter = self.collection_method_value(size_kind)?;
        let size_key = self.intern_property_key(COLLECTION_SIZE_NAME)?;
        self.objects.define_property(
            prototype,
            size_key,
            COLLECTION_SIZE_NAME,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.install_collection_to_string_tag(prototype, kind)?;
        self.install_collection_prototype_iterator(prototype, kind)
    }

    fn install_collection_to_string_tag(
        &mut self,
        prototype: ObjectId,
        kind: CollectionKind,
    ) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag_symbol = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(symbol) = tag_symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let tag = match kind {
            CollectionKind::Map => MAP_NAME,
            CollectionKind::Set => SET_NAME,
            _ => {
                return Err(Error::runtime("weak collection routed to Map or Set tag"));
            }
        };
        let tag = self.heap_string_value(tag)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol.id()),
            TO_STRING_TAG_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(tag),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    /// Installs `Symbol.iterator` on the prototype, aliasing `entries` for
    /// maps and `values` for sets.
    fn install_collection_prototype_iterator(
        &mut self,
        prototype: crate::value::ObjectId,
        kind: CollectionKind,
    ) -> Result<()> {
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        let iterator_kind = match kind {
            CollectionKind::Map => NativeFunctionKind::MapEntries,
            CollectionKind::Set => NativeFunctionKind::SetValues,
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to Map or Set iterator",
                ));
            }
        };
        let method = self.collection_method_value(iterator_kind)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol),
            ITERATOR_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(method),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        Ok(())
    }

    pub(in crate::runtime::native) fn construct_collection_object(
        &mut self,
        kind: CollectionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let constructor = self.collection_constructor_value(kind)?;
        let Value::NativeFunction(constructor_id) = &constructor else {
            return Err(Error::runtime("collection constructor disappeared"));
        };
        let prototype = self
            .native_function(*constructor_id)?
            .properties()
            .prototype();
        let Value::Object(prototype_id) = prototype else {
            return Err(Error::runtime("collection prototype is not an object"));
        };
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype_id),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("collection object creation failed"));
        };
        let collection = self.create_collection(kind)?;
        self.bind_collection_object(*object_id, kind, collection)?;
        let iterable = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !matches!(iterable, Value::Undefined | Value::Null) {
            self.seed_collection_from_iterable(kind, &object, &iterable)?;
        }
        Ok(object)
    }

    fn seed_collection_from_iterable(
        &mut self,
        kind: CollectionKind,
        object: &Value,
        iterable: &Value,
    ) -> Result<()> {
        let adder_name = match kind {
            CollectionKind::Map => COLLECTION_SET_NAME,
            CollectionKind::Set => COLLECTION_ADD_NAME,
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to Map or Set seeding",
                ));
            }
        };
        let adder = self.get_named(object, adder_name)?;
        if !self.semantic_is_callable(&adder)? {
            return Err(Error::type_error(COLLECTION_ADDER_ERROR));
        }
        let mut source = self.get_iterator(iterable)?;
        loop {
            match self.iterator_step(&mut source)? {
                crate::runtime::abstract_operations::IteratorStep::Value(item) => {
                    let outcome = self.seed_collection_entry(kind, object, &adder, item);
                    if let Err(error) = outcome {
                        return Err(self.iterator_close_on_error(&mut source, error));
                    }
                }
                crate::runtime::abstract_operations::IteratorStep::Done => return Ok(()),
                crate::runtime::abstract_operations::IteratorStep::Abrupt(completion) => {
                    return completion.into_result().map(|_| ());
                }
            }
        }
    }

    fn seed_collection_entry(
        &mut self,
        kind: CollectionKind,
        object: &Value,
        adder: &Value,
        item: Value,
    ) -> Result<()> {
        let args = match kind {
            CollectionKind::Map => {
                if !matches!(item, Value::Object(_)) {
                    return Err(Error::type_error(MAP_ENTRY_NOT_OBJECT_ERROR));
                }
                let key = self.get_named(&item, "0")?;
                let value = self.get_named(&item, "1")?;
                vec![key, value]
            }
            CollectionKind::Set => vec![item],
            _ => {
                return Err(Error::runtime(
                    "weak collection routed to Map or Set seeding",
                ));
            }
        };
        self.call_value(adder, &args, object.clone()).map(|_| ())
    }

    pub(in crate::runtime::native) fn eval_collection_constructor_call() -> Result<Value> {
        Err(Error::type_error(CONSTRUCTOR_REQUIRES_NEW_ERROR))
    }

    pub(in crate::runtime::native) fn eval_map_get(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::Map)?;
        let key = first_arg(&args);
        Ok(self
            .collection_get(collection, &key)?
            .unwrap_or(Value::Undefined))
    }

    pub(in crate::runtime::native) fn eval_map_set(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::Map)?;
        let key = first_arg(&args);
        let value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        self.collection_set(collection, key, value)?;
        Ok(this_value.clone())
    }

    pub(in crate::runtime::native) fn eval_set_add(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::Set)?;
        let value = first_arg(&args);
        self.collection_set(collection, value.clone(), value)?;
        Ok(this_value.clone())
    }

    pub(in crate::runtime::native) fn eval_collection_has(
        &self,
        kind: CollectionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, kind)?;
        let key = first_arg(&args);
        Ok(Value::Bool(self.collection_has(collection, &key)?))
    }

    pub(in crate::runtime::native) fn eval_collection_delete(
        &mut self,
        kind: CollectionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, kind)?;
        let key = first_arg(&args);
        Ok(Value::Bool(self.collection_delete(collection, &key)?))
    }

    pub(in crate::runtime::native) fn eval_collection_clear(
        &mut self,
        kind: CollectionKind,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, kind)?;
        self.collection_clear(collection)?;
        Ok(Value::Undefined)
    }

    pub(in crate::runtime::native) fn eval_collection_size(
        &self,
        kind: CollectionKind,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, kind)?;
        let len = self.collection_len(collection)?;
        let len = u32::try_from(len)
            .map_err(|_| Error::limit("collection size exceeded supported range"))?;
        Ok(Value::Number(f64::from(len)))
    }

    pub(in crate::runtime::native) fn eval_collection_for_each(
        &mut self,
        kind: CollectionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, kind)?;
        let callback = first_arg(&args);
        if !self.semantic_is_callable(&callback)? {
            return Err(Error::type_error(FOR_EACH_CALLBACK_ERROR));
        }
        let callback_this = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        let mut cursor = 0usize;
        while let Some((index, key, value)) =
            self.collection_entry_at_or_after(collection, cursor)?
        {
            cursor = index
                .checked_add(1)
                .ok_or_else(|| Error::limit("collection forEach cursor overflowed"))?;
            let call_args = [value, key, this_value.clone()];
            match self.call(&callback, &call_args, callback_this.clone())? {
                Completion::Normal(_) => {}
                completion => return completion.into_result(),
            }
        }
        Ok(Value::Undefined)
    }

    /// Materializes an iterator object over a snapshot of the collection.
    pub(in crate::runtime::native) fn eval_collection_iterator(
        &mut self,
        kind: CollectionKind,
        target: CollectionIterationTarget,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, kind)?;
        let iterator =
            self.create_live_collection_iterator(this_value.clone(), collection, kind, target)?;
        let tag = match kind {
            CollectionKind::Map => MAP_ITERATOR_TAG,
            CollectionKind::Set => SET_ITERATOR_TAG,
            _ => {
                return Err(Error::runtime("weak collection cannot have an iterator"));
            }
        };
        self.create_live_collection_iterator_object(iterator, tag)
    }

    pub(in crate::runtime::native) fn create_collection_iterator_object(
        &mut self,
        items: Vec<Value>,
    ) -> Result<Value> {
        // Array and snapshot iterators share one prototype that inherits the
        // ES2025 iterator helpers.
        let iterator_prototype = self.collection_iterator_prototype_id()?;
        let iterator_id = self.create_collection_iterator(items)?;
        let next = self.create_native_function(
            NativeFunctionKind::CollectionIteratorNext(iterator_id),
            Value::Undefined,
        )?;
        let next_key = self.intern_property_key(ITERATOR_NEXT_NAME)?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(iterator_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("iterator object creation failed"));
        };
        self.objects.define_property(
            *object_id,
            next_key,
            ITERATOR_NEXT_NAME,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(next.clone()),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.define_collection_iterator_state(*object_id, next)?;
        // Iterators are themselves iterable: [Symbol.iterator]() returns this.
        self.symbol_constructor_value()?;
        if let Some(symbol) = self.iterator_symbol() {
            let self_fn =
                self.create_native_function(NativeFunctionKind::IteratorSelf, Value::Undefined)?;
            self.objects.define_property(
                *object_id,
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
        }
        Ok(object)
    }

    pub(in crate::runtime::native) fn create_tagged_collection_iterator_object(
        &mut self,
        items: Vec<Value>,
        tag: &str,
    ) -> Result<Value> {
        let iterator_id = self.create_collection_iterator(items)?;
        self.create_tagged_iterator_state_object(iterator_id, tag)
    }

    pub(in crate::runtime::native) fn create_tagged_iterator_state_object(
        &mut self,
        iterator_id: CollectionIteratorId,
        tag: &str,
    ) -> Result<Value> {
        // Tagged per-kind prototypes chain to the shared iterator helpers.
        let iterator_prototype = self.iterator_prototype_object_id()?;
        let next = self.create_native_function(
            NativeFunctionKind::CollectionIteratorNext(iterator_id),
            Value::Undefined,
        )?;
        let next_key = self.intern_property_key(ITERATOR_NEXT_NAME)?;
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype(
            Some(iterator_prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype_id) = prototype else {
            return Err(Error::runtime("iterator prototype creation failed"));
        };
        self.objects.define_property(
            prototype_id,
            next_key,
            ITERATOR_NEXT_NAME,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(next.clone()),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.install_iterator_prototype_symbols(prototype_id, tag)?;
        let object = self.objects.create_with_prototype(
            Some(prototype_id),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = object else {
            return Err(Error::runtime("tagged iterator object creation failed"));
        };
        self.define_collection_iterator_state(object_id, next)?;
        Ok(Value::Object(object_id))
    }

    fn create_live_collection_iterator_object(
        &mut self,
        iterator: CollectionIteratorId,
        tag: &str,
    ) -> Result<Value> {
        let iterator_parent = self.iterator_prototype_object_id()?;
        let next = self.create_ephemeral_native_function(
            NativeFunctionKind::CollectionIteratorNext(iterator),
            Value::Undefined,
        )?;
        let next_key = self.intern_property_key(ITERATOR_NEXT_NAME)?;
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype(
            Some(iterator_parent),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype_id) = prototype else {
            return Err(Error::runtime("live iterator prototype creation failed"));
        };
        self.objects.define_property(
            prototype_id,
            next_key,
            ITERATOR_NEXT_NAME,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(next.clone()),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.install_iterator_prototype_symbols(prototype_id, tag)?;
        let object = self.objects.create_with_prototype(
            Some(prototype_id),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = object else {
            return Err(Error::runtime("live iterator object creation failed"));
        };
        self.define_collection_iterator_state(object_id, next)?;
        Ok(Value::Object(object_id))
    }

    pub(in crate::runtime::native) fn define_collection_iterator_state(
        &mut self,
        object: ObjectId,
        state: Value,
    ) -> Result<()> {
        let key = self.intern_property_key(COLLECTION_ITERATOR_STATE_PROPERTY)?;
        self.objects.define_property(
            object,
            key,
            COLLECTION_ITERATOR_STATE_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(state),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_iterator_prototype_symbols(
        &mut self,
        prototype: crate::value::ObjectId,
        tag: &str,
    ) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        if let Some(symbol) = self.iterator_symbol() {
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
        }
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
        )
    }

    pub(in crate::runtime::native) fn eval_collection_iterator_next(
        &mut self,
        iterator: CollectionIteratorId,
        this_value: &Value,
    ) -> Result<Value> {
        let actual = self.collection_iterator_receiver_state(this_value)?;
        self.eval_collection_iterator_next_state(iterator, actual)
    }

    pub(in crate::runtime::native) fn eval_collection_iterator_next_state(
        &mut self,
        iterator: CollectionIteratorId,
        actual: CollectionIteratorId,
    ) -> Result<Value> {
        let step = self.collection_iterator_step_for_receiver(iterator, actual)?;
        let (value, done) = step.map_or((Value::Undefined, true), |value| (value, false));
        let value_key = self.intern_property_key(ITERATOR_RESULT_VALUE_NAME)?;
        let done_key = self.intern_property_key(ITERATOR_RESULT_DONE_NAME)?;
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create(
            vec![
                ObjectPropertyInit::new(
                    value_key,
                    ITERATOR_RESULT_VALUE_NAME,
                    value,
                    PropertyEnumerable::Yes,
                ),
                ObjectPropertyInit::new(
                    done_key,
                    ITERATOR_RESULT_DONE_NAME,
                    Value::Bool(done),
                    PropertyEnumerable::Yes,
                ),
            ],
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn collection_iterator_receiver_state(
        &mut self,
        this_value: &Value,
    ) -> Result<CollectionIteratorId> {
        if !matches!(this_value, Value::Object(_)) {
            return Err(Error::type_error(COLLECTION_ITERATOR_RECEIVER_ERROR));
        }
        let key = self.intern_property_key(COLLECTION_ITERATOR_STATE_PROPERTY)?;
        let property =
            DynamicPropertyKey::new(COLLECTION_ITERATOR_STATE_PROPERTY.to_owned(), Some(key));
        let Some(OwnPropertyDescriptor::Data(descriptor)) =
            self.semantic_own_property_descriptor(this_value, &property)?
        else {
            return Err(Error::type_error(COLLECTION_ITERATOR_RECEIVER_ERROR));
        };
        let Value::NativeFunction(id) = descriptor.value() else {
            return Err(Error::type_error(COLLECTION_ITERATOR_RECEIVER_ERROR));
        };
        let NativeFunctionKind::CollectionIteratorNext(iterator) = self.native_function(id)?.kind()
        else {
            return Err(Error::type_error(COLLECTION_ITERATOR_RECEIVER_ERROR));
        };
        Ok(iterator)
    }
}

fn first_arg(args: &RuntimeCallArgs<'_>) -> Value {
    args.as_slice().first().cloned().unwrap_or(Value::Undefined)
}
