use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{IteratorStep, same_value},
        call::RuntimeCallArgs,
        collections::{CollectionId, CollectionKind, canonicalize_keyed_collection_key},
        object::{DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyWritable},
        roots::VmRootKind,
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use super::NativeFunctionKind;

const MAP_GROUP_BY_NAME: &str = "groupBy";
const MAP_GET_OR_INSERT_NAME: &str = "getOrInsert";
const MAP_GET_OR_INSERT_COMPUTED_NAME: &str = "getOrInsertComputed";
const MAP_GROUP_BY_CALLBACK_ERROR: &str = "Map.groupBy callback must be callable";
const MAP_GET_OR_INSERT_CALLBACK_ERROR: &str =
    "Map.prototype.getOrInsertComputed callback must be callable";
const MAP_GROUP_BY_INDEX_ERROR: &str = "Map.groupBy iteration index exceeded safe integer range";
const MAP_GROUP_BY_STORAGE_ERROR: &str = "Map.groupBy temporary group storage exhausted";
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

impl Context {
    pub(in crate::runtime) fn eval_modern_map_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::MapGroupBy => Some(self.eval_map_group_by(args)),
            NativeFunctionKind::MapGetOrInsert => {
                Some(self.eval_map_get_or_insert(args, this_value))
            }
            NativeFunctionKind::MapGetOrInsertComputed => {
                Some(self.eval_map_get_or_insert_computed(args, this_value))
            }
            _ => None,
        }
    }

    pub(in crate::runtime::native) fn install_modern_map_constructor_methods(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        let method = self
            .create_ephemeral_native_function(NativeFunctionKind::MapGroupBy, Value::Undefined)?;
        let key = self.intern_property_key(MAP_GROUP_BY_NAME)?;
        self.define_native_function_property_key(
            constructor,
            MAP_GROUP_BY_NAME,
            key,
            builtin_method_update(method),
        )
    }

    pub(in crate::runtime::native) fn install_modern_map_prototype_methods(
        &mut self,
        prototype: ObjectId,
    ) -> Result<()> {
        for (name, kind) in [
            (MAP_GET_OR_INSERT_NAME, NativeFunctionKind::MapGetOrInsert),
            (
                MAP_GET_OR_INSERT_COMPUTED_NAME,
                NativeFunctionKind::MapGetOrInsertComputed,
            ),
        ] {
            let method = self.create_ephemeral_native_function(kind, Value::Undefined)?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        Ok(())
    }

    pub(in crate::runtime::native) fn eval_map_group_by(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let items = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let callback = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&callback)? {
            return Err(Error::type_error(MAP_GROUP_BY_CALLBACK_ERROR));
        }
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        let mut iterator = self.get_iterator(&items)?;
        let mut groups: Vec<(Value, Vec<Value>)> = Vec::new();
        let mut index = 0.0;
        loop {
            if index >= MAX_SAFE_INTEGER {
                let error = Error::type_error(MAP_GROUP_BY_INDEX_ERROR);
                return Err(self.iterator_close_on_error(&mut iterator, error));
            }
            let value = match self.iterator_step(&mut iterator)? {
                IteratorStep::Value(value) => value,
                IteratorStep::Done => break,
                IteratorStep::Abrupt(completion) => return completion.into_result(),
            };
            let callback_args = [value.clone(), Value::Number(index)];
            let key = match self.call_value(&callback, &callback_args, Value::Undefined) {
                Ok(key) => canonicalize_keyed_collection_key(key),
                Err(error) => return Err(self.iterator_close_on_error(&mut iterator, error)),
            };
            if let Err(error) = roots.add_values([&value, &key]) {
                return Err(self.iterator_close_on_error(&mut iterator, error));
            }
            if let Some((_group_key, values)) = groups
                .iter_mut()
                .find(|(group_key, _values)| same_value(group_key, &key))
            {
                if let Err(error) = values.try_reserve(1) {
                    let error = Error::limit(format!("{MAP_GROUP_BY_STORAGE_ERROR}: {error}"));
                    return Err(self.iterator_close_on_error(&mut iterator, error));
                }
                values.push(value);
            } else {
                if let Err(error) = groups.try_reserve(1) {
                    let error = Error::limit(format!("{MAP_GROUP_BY_STORAGE_ERROR}: {error}"));
                    return Err(self.iterator_close_on_error(&mut iterator, error));
                }
                groups.push((key, vec![value]));
            }
            index += 1.0;
        }

        let (result, result_id) = self.new_map_object()?;
        roots.add_values(std::iter::once(&result))?;
        for (key, values) in groups {
            let group = self.create_array_from_elements(values)?;
            roots.add_values(std::iter::once(&group))?;
            self.collection_set(result_id, key, group)?;
        }
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_map_get_or_insert(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::Map)?;
        let key = canonicalize_keyed_collection_key(
            args.as_slice().first().cloned().unwrap_or(Value::Undefined),
        );
        if let Some(value) = self.collection_get(collection, &key)? {
            return Ok(value);
        }
        let value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        self.collection_set(collection, key, value.clone())?;
        Ok(value)
    }

    pub(in crate::runtime::native) fn eval_map_get_or_insert_computed(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let collection = self.collection_from_this(this_value, CollectionKind::Map)?;
        let callback = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if !self.semantic_is_callable(&callback)? {
            return Err(Error::type_error(MAP_GET_OR_INSERT_CALLBACK_ERROR));
        }
        let key = canonicalize_keyed_collection_key(
            args.as_slice().first().cloned().unwrap_or(Value::Undefined),
        );
        if let Some(value) = self.collection_get(collection, &key)? {
            return Ok(value);
        }
        let value = self.call_value(&callback, std::slice::from_ref(&key), Value::Undefined)?;
        self.collection_set(collection, key, value.clone())?;
        Ok(value)
    }

    fn new_map_object(&mut self) -> Result<(Value, CollectionId)> {
        let constructor = self.collection_constructor_value(CollectionKind::Map)?;
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime("Map constructor is not native"));
        };
        let prototype = self
            .native_function(constructor_id)?
            .properties()
            .prototype();
        let Value::Object(prototype_id) = prototype else {
            return Err(Error::runtime("Map prototype is not an object"));
        };
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype_id),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("Map object creation failed"));
        };
        let collection = self.create_collection(CollectionKind::Map)?;
        self.bind_collection_object(*object_id, CollectionKind::Map, collection)?;
        Ok((object, collection))
    }
}

const fn builtin_method_update(value: Value) -> DataPropertyUpdate {
    DataPropertyUpdate::new(
        Some(value),
        Some(PropertyWritable::Yes),
        Some(PropertyEnumerable::No),
        Some(PropertyConfigurable::Yes),
    )
}
