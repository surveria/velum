use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        collections::{CollectionId, CollectionKind},
        control::Completion,
    },
    value::{ErrorName, Value},
};

const SET_SIZE_PROPERTY: &str = "size";
const SET_HAS_PROPERTY: &str = "has";
const SET_KEYS_PROPERTY: &str = "keys";
const ITERATOR_NEXT_PROPERTY: &str = "next";
const ITERATOR_DONE_PROPERTY: &str = "done";
const ITERATOR_VALUE_PROPERTY: &str = "value";
const SET_LIKE_NOT_OBJECT_ERROR: &str = "Set method argument must be a set-like object";
const SET_LIKE_SIZE_NAN_ERROR: &str = "Set-like argument size must be a number";
const SET_LIKE_SIZE_NEGATIVE_ERROR: &str = "Set-like argument size must not be negative";
const SET_LIKE_HAS_ERROR: &str = "Set-like argument has method must be callable";
const SET_LIKE_KEYS_ERROR: &str = "Set-like argument keys method must be callable";
const SET_KEYS_ITERATOR_ERROR: &str = "Set-like argument keys method must return an iterator";
const SET_KEYS_RESULT_ERROR: &str = "Set-like iterator must return an object result";

/// Validated `GetSetRecord` describing a set-like argument.
struct SetRecord {
    object: Value,
    size: f64,
    has: Value,
    keys: Value,
}

impl Context {
    pub(in crate::runtime::native) fn eval_set_union(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let this_id = self.collection_from_this(this_value, CollectionKind::Set)?;
        let record = self.get_set_record(&Self::set_first_arg(&args))?;
        let (result, result_id) = self.new_set_object()?;
        for item in self.set_items(this_id)? {
            self.collection_set(result_id, item.clone(), item)?;
        }
        for key in self.set_record_keys(&record)? {
            self.step()?;
            self.collection_set(result_id, key.clone(), key)?;
        }
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_set_intersection(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let this_id = self.collection_from_this(this_value, CollectionKind::Set)?;
        let record = self.get_set_record(&Self::set_first_arg(&args))?;
        let (result, result_id) = self.new_set_object()?;
        if Self::set_size_as_f64(self.collection_len(this_id)?) <= record.size {
            for item in self.set_items(this_id)? {
                self.step()?;
                if self.set_record_has(&record, &item)? {
                    self.collection_set(result_id, item.clone(), item)?;
                }
            }
        } else {
            for key in self.set_record_keys(&record)? {
                self.step()?;
                if self.collection_has(this_id, &key)? && !self.collection_has(result_id, &key)? {
                    self.collection_set(result_id, key.clone(), key)?;
                }
            }
        }
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_set_difference(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let this_id = self.collection_from_this(this_value, CollectionKind::Set)?;
        let record = self.get_set_record(&Self::set_first_arg(&args))?;
        let (result, result_id) = self.new_set_object()?;
        let items = self.set_items(this_id)?;
        for item in &items {
            self.collection_set(result_id, item.clone(), item.clone())?;
        }
        if Self::set_size_as_f64(self.collection_len(this_id)?) <= record.size {
            for item in items {
                self.step()?;
                if self.set_record_has(&record, &item)? {
                    self.collection_delete(result_id, &item)?;
                }
            }
        } else {
            for key in self.set_record_keys(&record)? {
                self.step()?;
                if self.collection_has(result_id, &key)? {
                    self.collection_delete(result_id, &key)?;
                }
            }
        }
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_set_symmetric_difference(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let this_id = self.collection_from_this(this_value, CollectionKind::Set)?;
        let record = self.get_set_record(&Self::set_first_arg(&args))?;
        let (result, result_id) = self.new_set_object()?;
        for item in self.set_items(this_id)? {
            self.collection_set(result_id, item.clone(), item)?;
        }
        for key in self.set_record_keys(&record)? {
            self.step()?;
            if self.collection_has(this_id, &key)? {
                self.collection_delete(result_id, &key)?;
            } else if !self.collection_has(result_id, &key)? {
                self.collection_set(result_id, key.clone(), key)?;
            }
        }
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_set_is_subset_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let this_id = self.collection_from_this(this_value, CollectionKind::Set)?;
        let record = self.get_set_record(&Self::set_first_arg(&args))?;
        if Self::set_size_as_f64(self.collection_len(this_id)?) > record.size {
            return Ok(Value::Bool(false));
        }
        for item in self.set_items(this_id)? {
            self.step()?;
            if !self.set_record_has(&record, &item)? {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    pub(in crate::runtime::native) fn eval_set_is_superset_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let this_id = self.collection_from_this(this_value, CollectionKind::Set)?;
        let record = self.get_set_record(&Self::set_first_arg(&args))?;
        if Self::set_size_as_f64(self.collection_len(this_id)?) < record.size {
            return Ok(Value::Bool(false));
        }
        for key in self.set_record_keys(&record)? {
            self.step()?;
            if !self.collection_has(this_id, &key)? {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::Bool(true))
    }

    pub(in crate::runtime::native) fn eval_set_is_disjoint_from(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let this_id = self.collection_from_this(this_value, CollectionKind::Set)?;
        let record = self.get_set_record(&Self::set_first_arg(&args))?;
        if Self::set_size_as_f64(self.collection_len(this_id)?) <= record.size {
            for item in self.set_items(this_id)? {
                self.step()?;
                if self.set_record_has(&record, &item)? {
                    return Ok(Value::Bool(false));
                }
            }
        } else {
            for key in self.set_record_keys(&record)? {
                self.step()?;
                if self.collection_has(this_id, &key)? {
                    return Ok(Value::Bool(false));
                }
            }
        }
        Ok(Value::Bool(true))
    }

    fn set_first_arg(args: &RuntimeCallArgs<'_>) -> Value {
        args.as_slice().first().cloned().unwrap_or(Value::Undefined)
    }

    /// Snapshot of the values held by a Set backing store, in insertion order.
    fn set_items(&self, collection: CollectionId) -> Result<Vec<Value>> {
        Ok(self
            .collection_entries(collection)?
            .into_iter()
            .map(|(key, _)| key)
            .collect())
    }

    fn set_size_as_f64(size: usize) -> f64 {
        u32::try_from(size).map_or(f64::INFINITY, f64::from)
    }

    /// Spec `GetSetRecord(obj)`: validate a set-like argument.
    fn get_set_record(&mut self, other: &Value) -> Result<SetRecord> {
        if !matches!(other, Value::Object(_)) {
            return Err(Error::type_error(SET_LIKE_NOT_OBJECT_ERROR));
        }
        let raw_size = self.get_property_value(other, SET_SIZE_PROPERTY)?;
        let size = Self::value_to_number(&raw_size);
        if size.is_nan() {
            return Err(Error::type_error(SET_LIKE_SIZE_NAN_ERROR));
        }
        let size = if size.is_infinite() {
            size
        } else {
            size.trunc()
        };
        if size < 0.0 {
            return Err(Error::exception(
                ErrorName::RangeError,
                SET_LIKE_SIZE_NEGATIVE_ERROR,
            ));
        }
        let has = self.get_property_value(other, SET_HAS_PROPERTY)?;
        if !self.semantic_is_callable(&has)? {
            return Err(Error::type_error(SET_LIKE_HAS_ERROR));
        }
        let keys = self.get_property_value(other, SET_KEYS_PROPERTY)?;
        if !self.semantic_is_callable(&keys)? {
            return Err(Error::type_error(SET_LIKE_KEYS_ERROR));
        }
        Ok(SetRecord {
            object: other.clone(),
            size,
            has,
            keys,
        })
    }

    fn set_record_has(&mut self, record: &SetRecord, value: &Value) -> Result<bool> {
        let args = [value.clone()];
        let result = self.set_call(&record.has, &args, record.object.clone())?;
        Ok(result.is_truthy())
    }

    /// Drive the iterator returned by the set-like `keys` method to completion.
    fn set_record_keys(&mut self, record: &SetRecord) -> Result<Vec<Value>> {
        let iterator = self.set_call(&record.keys, &[], record.object.clone())?;
        if !matches!(iterator, Value::Object(_)) {
            return Err(Error::type_error(SET_KEYS_ITERATOR_ERROR));
        }
        let next = self.get_property_value(&iterator, ITERATOR_NEXT_PROPERTY)?;
        let mut values = Vec::new();
        loop {
            self.step()?;
            let result = self.set_call(&next, &[], iterator.clone())?;
            if !matches!(result, Value::Object(_)) {
                return Err(Error::type_error(SET_KEYS_RESULT_ERROR));
            }
            let done = self.get_property_value(&result, ITERATOR_DONE_PROPERTY)?;
            if done.is_truthy() {
                break;
            }
            values.push(self.get_property_value(&result, ITERATOR_VALUE_PROPERTY)?);
        }
        Ok(values)
    }

    fn set_call(&mut self, callee: &Value, args: &[Value], this_value: Value) -> Result<Value> {
        match self.eval_call_completion(callee, args, this_value)? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    /// Create a fresh `Set` object bound to a new empty backing store.
    fn new_set_object(&mut self) -> Result<(Value, CollectionId)> {
        let constructor = self.collection_constructor_value(CollectionKind::Set)?;
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime("Set constructor is not native"));
        };
        let prototype = self
            .native_function(constructor_id)?
            .properties()
            .prototype();
        let Value::Object(prototype_id) = prototype else {
            return Err(Error::runtime("Set prototype is not an object"));
        };
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype_id),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("Set object creation failed"));
        };
        let collection = self.create_collection()?;
        self.bind_collection_object(*object_id, CollectionKind::Set, collection)?;
        Ok((object, collection))
    }
}
