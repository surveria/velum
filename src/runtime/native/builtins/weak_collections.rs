use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        collections::{CollectionId, CollectionKind},
        object::{
            DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyUpdate, PropertyWritable,
        },
    },
    value::{ObjectId, Value},
};

use super::{NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY};

pub(in crate::runtime::native) const WEAK_MAP_NAME: &str = "WeakMap";
pub(in crate::runtime::native) const WEAK_SET_NAME: &str = "WeakSet";
const WEAK_COLLECTION_GET_NAME: &str = "get";
const WEAK_COLLECTION_SET_NAME: &str = "set";
const WEAK_COLLECTION_ADD_NAME: &str = "add";
const WEAK_COLLECTION_HAS_NAME: &str = "has";
const WEAK_COLLECTION_DELETE_NAME: &str = "delete";
const WEAK_MAP_GET_OR_INSERT_NAME: &str = "getOrInsert";
const WEAK_MAP_GET_OR_INSERT_COMPUTED_NAME: &str = "getOrInsertComputed";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const TO_STRING_TAG_SYMBOL_DISPLAY: &str = "[Symbol.toStringTag]";
const WEAK_MAP_ENTRY_NOT_OBJECT_ERROR: &str = "WeakMap iterable entries must be objects";
const WEAK_KEY_ERROR: &str = "WeakMap and WeakSet keys must be objects or symbols";
const WEAK_COLLECTION_ADDER_ERROR: &str = "weak collection adder must be callable";
const WEAK_MAP_CALLBACK_ERROR: &str =
    "WeakMap.prototype.getOrInsertComputed callback must be callable";

impl Context {
    pub(in crate::runtime::native) fn weak_map_constructor_value(&mut self) -> Result<Value> {
        self.weak_collection_constructor_value(CollectionKind::WeakMap)
    }

    pub(in crate::runtime::native) fn weak_set_constructor_value(&mut self) -> Result<Value> {
        self.weak_collection_constructor_value(CollectionKind::WeakSet)
    }

    fn weak_collection_constructor_value(&mut self, kind: CollectionKind) -> Result<Value> {
        let constructor_kind = match kind {
            CollectionKind::WeakMap => NativeFunctionKind::WeakMap,
            CollectionKind::WeakSet => NativeFunctionKind::WeakSet,
            CollectionKind::Map
            | CollectionKind::Set
            | CollectionKind::AsyncDisposableStack
            | CollectionKind::DisposableStack => {
                return Err(Error::runtime(
                    "strong collection routed to WeakMap or WeakSet",
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
        self.install_weak_collection_prototype_methods(prototype, kind)?;
        let global_name = match kind {
            CollectionKind::WeakMap => WEAK_MAP_NAME,
            CollectionKind::WeakSet => WEAK_SET_NAME,
            CollectionKind::Map
            | CollectionKind::Set
            | CollectionKind::AsyncDisposableStack
            | CollectionKind::DisposableStack => {
                return Err(Error::runtime(
                    "strong collection routed to WeakMap or WeakSet",
                ));
            }
        };
        self.insert_global_builtin(global_name, constructor.clone())?;
        Ok(constructor)
    }

    fn weak_collection_method_value(&mut self, kind: NativeFunctionKind) -> Result<Value> {
        if let Some(id) = self.native_function_id(kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.create_native_function(kind, Value::Undefined)
    }

    fn install_weak_collection_prototype_methods(
        &mut self,
        prototype: crate::value::ObjectId,
        kind: CollectionKind,
    ) -> Result<()> {
        let methods: &[(&str, NativeFunctionKind)] = match kind {
            CollectionKind::WeakMap => &[
                (WEAK_COLLECTION_GET_NAME, NativeFunctionKind::WeakMapGet),
                (WEAK_COLLECTION_SET_NAME, NativeFunctionKind::WeakMapSet),
                (WEAK_COLLECTION_HAS_NAME, NativeFunctionKind::WeakMapHas),
                (
                    WEAK_COLLECTION_DELETE_NAME,
                    NativeFunctionKind::WeakMapDelete,
                ),
            ],
            CollectionKind::WeakSet => &[
                (WEAK_COLLECTION_ADD_NAME, NativeFunctionKind::WeakSetAdd),
                (WEAK_COLLECTION_HAS_NAME, NativeFunctionKind::WeakSetHas),
                (
                    WEAK_COLLECTION_DELETE_NAME,
                    NativeFunctionKind::WeakSetDelete,
                ),
            ],
            CollectionKind::Map
            | CollectionKind::Set
            | CollectionKind::AsyncDisposableStack
            | CollectionKind::DisposableStack => {
                return Err(Error::runtime(
                    "strong collection routed to WeakMap or WeakSet",
                ));
            }
        };
        for (name, method_kind) in methods {
            let method = self.weak_collection_method_value(*method_kind)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        if kind == CollectionKind::WeakMap {
            self.install_modern_weak_map_methods(prototype)?;
        }
        self.install_weak_collection_to_string_tag(prototype, kind)
    }

    fn install_modern_weak_map_methods(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (
                WEAK_MAP_GET_OR_INSERT_NAME,
                NativeFunctionKind::WeakMapGetOrInsert,
            ),
            (
                WEAK_MAP_GET_OR_INSERT_COMPUTED_NAME,
                NativeFunctionKind::WeakMapGetOrInsertComputed,
            ),
        ] {
            let method = self.create_ephemeral_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        Ok(())
    }

    fn install_weak_collection_to_string_tag(
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
            CollectionKind::WeakMap => WEAK_MAP_NAME,
            CollectionKind::WeakSet => WEAK_SET_NAME,
            CollectionKind::Map
            | CollectionKind::Set
            | CollectionKind::AsyncDisposableStack
            | CollectionKind::DisposableStack => {
                return Err(Error::runtime(
                    "strong collection routed to WeakMap or WeakSet tag",
                ));
            }
        };
        let value = self.heap_string_value(tag)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(symbol.id()),
            TO_STRING_TAG_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn construct_weak_collection_object(
        &mut self,
        kind: CollectionKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let constructor = self.weak_collection_constructor_value(kind)?;
        let Value::NativeFunction(constructor_id) = &constructor else {
            return Err(Error::runtime("weak collection constructor disappeared"));
        };
        let prototype = self
            .native_function(*constructor_id)?
            .properties()
            .prototype();
        let Value::Object(prototype_id) = prototype else {
            return Err(Error::runtime("weak collection prototype is not an object"));
        };
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype_id),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("weak collection object creation failed"));
        };
        let collection = self.create_collection(kind)?;
        self.bind_collection_object(*object_id, kind, collection)?;
        let iterable = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !matches!(iterable, Value::Undefined | Value::Null) {
            self.seed_weak_collection_from_iterable(kind, &object, &iterable)?;
        }
        Ok(object)
    }

    fn seed_weak_collection_from_iterable(
        &mut self,
        kind: CollectionKind,
        object: &Value,
        iterable: &Value,
    ) -> Result<()> {
        let adder_name = match kind {
            CollectionKind::WeakMap => WEAK_COLLECTION_SET_NAME,
            CollectionKind::WeakSet => WEAK_COLLECTION_ADD_NAME,
            CollectionKind::Map
            | CollectionKind::Set
            | CollectionKind::AsyncDisposableStack
            | CollectionKind::DisposableStack => {
                return Err(Error::runtime(
                    "strong collection routed to WeakMap or WeakSet seeding",
                ));
            }
        };
        let adder = self.get_named(object, adder_name)?;
        if !self.semantic_is_callable(&adder)? {
            return Err(Error::type_error(WEAK_COLLECTION_ADDER_ERROR));
        }
        let mut source = self.get_iterator(iterable)?;
        loop {
            match self.iterator_step(&mut source)? {
                crate::runtime::abstract_operations::IteratorStep::Value(item) => {
                    let outcome = self.seed_weak_collection_entry(kind, object, &adder, item);
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

    fn seed_weak_collection_entry(
        &mut self,
        kind: CollectionKind,
        object: &Value,
        adder: &Value,
        item: Value,
    ) -> Result<()> {
        let args = match kind {
            CollectionKind::WeakMap => {
                if !matches!(item, Value::Object(_)) {
                    return Err(Error::type_error(WEAK_MAP_ENTRY_NOT_OBJECT_ERROR));
                }
                let key = self.get_named(&item, "0")?;
                let value = self.get_named(&item, "1")?;
                vec![key, value]
            }
            CollectionKind::WeakSet => vec![item],
            CollectionKind::Map
            | CollectionKind::Set
            | CollectionKind::AsyncDisposableStack
            | CollectionKind::DisposableStack => {
                return Err(Error::runtime(
                    "strong collection routed to WeakMap or WeakSet seeding",
                ));
            }
        };
        self.call_value(adder, &args, object.clone()).map(|_| ())
    }

    pub(in crate::runtime) fn eval_modern_collection_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        if let Some(result) = self.eval_modern_map_native_function_kind(kind, args, this_value) {
            return Some(result);
        }
        self.eval_modern_weak_map_native_function_kind(kind, args, this_value)
    }

    fn eval_modern_weak_map_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::WeakMapGetOrInsert => {
                Some(self.eval_weak_map_get_or_insert(args, this_value))
            }
            NativeFunctionKind::WeakMapGetOrInsertComputed => {
                Some(self.eval_weak_map_get_or_insert_computed(args, this_value))
            }
            _ => None,
        }
    }

    fn eval_weak_map_get_or_insert(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::WeakMap)?;
        let key = first_arg(&args);
        if !self.can_be_held_weakly(&key)? {
            return Err(Error::type_error(WEAK_KEY_ERROR));
        }
        if let Some(value) = self.collection_get(collection, &key)? {
            return Ok(value);
        }
        let value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        self.collection_set(collection, key, value.clone())?;
        Ok(value)
    }

    fn eval_weak_map_get_or_insert_computed(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::WeakMap)?;
        let callback = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&callback)? {
            return Err(Error::type_error(WEAK_MAP_CALLBACK_ERROR));
        }
        let key = first_arg(&args);
        if !self.can_be_held_weakly(&key)? {
            return Err(Error::type_error(WEAK_KEY_ERROR));
        }
        if let Some(value) = self.collection_get(collection, &key)? {
            return Ok(value);
        }
        let value = self.call_value(&callback, std::slice::from_ref(&key), Value::Undefined)?;
        self.collection_set(collection, key, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::native) fn eval_weak_map_get(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::WeakMap)?;
        let key = first_arg(&args);
        if !self.can_be_held_weakly(&key)? {
            return Ok(Value::Undefined);
        }
        Ok(self
            .collection_get(collection, &key)?
            .unwrap_or(Value::Undefined))
    }

    pub(in crate::runtime::native) fn eval_weak_map_set(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::WeakMap)?;
        let key = first_arg(&args);
        let value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        self.weak_map_set_entry(collection, key, value)?;
        Ok(this_value.clone())
    }

    pub(in crate::runtime::native) fn eval_weak_set_add(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::WeakSet)?;
        let value = first_arg(&args);
        self.weak_set_add_entry(collection, value)?;
        Ok(this_value.clone())
    }

    pub(in crate::runtime::native) fn eval_weak_collection_has(
        &self,
        kind: CollectionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, kind)?;
        let key = first_arg(&args);
        if !self.can_be_held_weakly(&key)? {
            return Ok(Value::Bool(false));
        }
        Ok(Value::Bool(self.collection_has(collection, &key)?))
    }

    pub(in crate::runtime::native) fn eval_weak_collection_delete(
        &mut self,
        kind: CollectionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, kind)?;
        let key = first_arg(&args);
        if !self.can_be_held_weakly(&key)? {
            return Ok(Value::Bool(false));
        }
        Ok(Value::Bool(self.collection_delete(collection, &key)?))
    }

    fn weak_map_set_entry(
        &mut self,
        collection: CollectionId,
        key: Value,
        value: Value,
    ) -> Result<()> {
        if !self.can_be_held_weakly(&key)? {
            return Err(Error::type_error(WEAK_KEY_ERROR));
        }
        self.collection_set(collection, key, value)
    }

    fn weak_set_add_entry(&mut self, collection: CollectionId, value: Value) -> Result<()> {
        if !self.can_be_held_weakly(&value)? {
            return Err(Error::type_error(WEAK_KEY_ERROR));
        }
        self.collection_set(collection, value.clone(), value)
    }

    fn can_be_held_weakly(&self, value: &Value) -> Result<bool> {
        match value {
            Value::Object(_) => Ok(true),
            Value::Symbol(symbol) => Ok(self.symbols.key_for(symbol.id())?.is_none()),
            _ => Ok(false),
        }
    }
}

fn first_arg(args: &RuntimeCallArgs<'_>) -> Value {
    args.as_slice().first().cloned().unwrap_or(Value::Undefined)
}
