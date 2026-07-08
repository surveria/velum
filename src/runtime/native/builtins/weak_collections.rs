use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        collections::{CollectionId, CollectionKind},
        object::{ObjectPropertyInit, PropertyEnumerable},
    },
    value::Value,
};

use super::{NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY};

pub(in crate::runtime::native) const WEAK_MAP_NAME: &str = "WeakMap";
pub(in crate::runtime::native) const WEAK_SET_NAME: &str = "WeakSet";
const WEAK_COLLECTION_GET_NAME: &str = "get";
const WEAK_COLLECTION_SET_NAME: &str = "set";
const WEAK_COLLECTION_ADD_NAME: &str = "add";
const WEAK_COLLECTION_HAS_NAME: &str = "has";
const WEAK_COLLECTION_DELETE_NAME: &str = "delete";
const WEAK_MAP_ENTRY_NOT_OBJECT_ERROR: &str = "WeakMap iterable entries must be objects";
const WEAK_KEY_ERROR: &str = "WeakMap and WeakSet keys must be objects or symbols";

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
            CollectionKind::Map | CollectionKind::Set => {
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
            CollectionKind::Map | CollectionKind::Set => {
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
            CollectionKind::Map | CollectionKind::Set => {
                return Err(Error::runtime(
                    "strong collection routed to WeakMap or WeakSet",
                ));
            }
        };
        for (name, method_kind) in methods {
            let method = self.weak_collection_method_value(*method_kind)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        Ok(())
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
        let collection = self.create_collection()?;
        self.bind_collection_object(*object_id, kind, collection)?;
        let iterable = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !matches!(iterable, Value::Undefined | Value::Null) {
            self.seed_weak_collection_from_iterable(kind, collection, &iterable)?;
        }
        Ok(object)
    }

    fn seed_weak_collection_from_iterable(
        &mut self,
        kind: CollectionKind,
        collection: CollectionId,
        iterable: &Value,
    ) -> Result<()> {
        let mut source = self.for_of_source(iterable.clone())?;
        loop {
            match self.for_of_step(&mut source)? {
                crate::runtime::bytecode::for_of::ForOfStep::Value(item) => {
                    let outcome = self.seed_weak_collection_entry(kind, collection, item);
                    if let Err(error) = outcome {
                        self.close_for_of_source(&source);
                        return Err(error);
                    }
                }
                crate::runtime::bytecode::for_of::ForOfStep::Done => return Ok(()),
                crate::runtime::bytecode::for_of::ForOfStep::Abrupt(completion) => {
                    return completion.into_result().map(|_| ());
                }
            }
        }
    }

    fn seed_weak_collection_entry(
        &mut self,
        kind: CollectionKind,
        collection: CollectionId,
        item: Value,
    ) -> Result<()> {
        match kind {
            CollectionKind::WeakMap => {
                if !matches!(item, Value::Object(_)) {
                    return Err(Error::type_error(WEAK_MAP_ENTRY_NOT_OBJECT_ERROR));
                }
                let key = self.get_property_value(&item, "0")?;
                let value = self.get_property_value(&item, "1")?;
                self.weak_map_set_entry(collection, key, value)
            }
            CollectionKind::WeakSet => self.weak_set_add_entry(collection, item),
            CollectionKind::Map | CollectionKind::Set => Err(Error::runtime(
                "strong collection routed to WeakMap or WeakSet",
            )),
        }
    }

    pub(in crate::runtime::native) fn eval_weak_map_get(
        &self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::WeakMap)?;
        let key = first_arg(&args);
        if !Self::can_be_held_weakly(&key) {
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
        if !Self::can_be_held_weakly(&key) {
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
        if !Self::can_be_held_weakly(&key) {
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
        if !Self::can_be_held_weakly(&key) {
            return Err(Error::type_error(WEAK_KEY_ERROR));
        }
        self.collection_set(collection, key, value)
    }

    fn weak_set_add_entry(&mut self, collection: CollectionId, value: Value) -> Result<()> {
        if !Self::can_be_held_weakly(&value) {
            return Err(Error::type_error(WEAK_KEY_ERROR));
        }
        self.collection_set(collection, value.clone(), value)
    }
}

fn first_arg(args: &RuntimeCallArgs<'_>) -> Value {
    args.as_slice().first().cloned().unwrap_or(Value::Undefined)
}
