use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{IteratorStep, integer_or_infinity_from_number, to_boolean},
        call::RuntimeCallArgs,
        collections::{CollectionId, CollectionKind},
    },
    value::{ErrorName, Value},
};

const SET_SIZE_PROPERTY: &str = "size";
const SET_HAS_PROPERTY: &str = "has";
const SET_KEYS_PROPERTY: &str = "keys";
const SET_LIKE_NOT_OBJECT_ERROR: &str = "Set method argument must be a set-like object";
const SET_LIKE_SIZE_NAN_ERROR: &str = "Set-like argument size must be a number";
const SET_LIKE_SIZE_NEGATIVE_ERROR: &str = "Set-like argument size must not be negative";

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
        self.with_collection_cursor_pin(this_id, |context| {
            let mut cursor = 0usize;
            while let Some((index, item, _value)) =
                context.collection_entry_at_or_after(this_id, cursor)?
            {
                cursor = index
                    .checked_add(1)
                    .ok_or_else(|| Error::limit("Set subset cursor overflowed"))?;
                context.step()?;
                if !context.set_record_has(&record, &item)? {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        })
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
        let mut source = self.get_iterator_from_method(&record.object, &record.keys)?;
        loop {
            self.step()?;
            match self.iterator_step(&mut source)? {
                IteratorStep::Value(key) if !self.collection_has(this_id, &key)? => {
                    return self.close_set_predicate_iterator(&mut source, false);
                }
                IteratorStep::Value(_) => {}
                IteratorStep::Done => return Ok(Value::Bool(true)),
                IteratorStep::Abrupt(completion) => return completion.into_result(),
            }
        }
    }

    pub(in crate::runtime::native) fn eval_set_is_disjoint_from(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let this_id = self.collection_from_this(this_value, CollectionKind::Set)?;
        let record = self.get_set_record(&Self::set_first_arg(&args))?;
        if Self::set_size_as_f64(self.collection_len(this_id)?) <= record.size {
            self.with_collection_cursor_pin(this_id, |context| {
                let mut cursor = 0usize;
                while let Some((index, item, _value)) =
                    context.collection_entry_at_or_after(this_id, cursor)?
                {
                    cursor = index
                        .checked_add(1)
                        .ok_or_else(|| Error::limit("Set disjoint cursor overflowed"))?;
                    context.step()?;
                    if context.set_record_has(&record, &item)? {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            })
        } else {
            let mut source = self.get_iterator_from_method(&record.object, &record.keys)?;
            loop {
                self.step()?;
                match self.iterator_step(&mut source)? {
                    IteratorStep::Value(key) if self.collection_has(this_id, &key)? => {
                        return self.close_set_predicate_iterator(&mut source, false);
                    }
                    IteratorStep::Value(_) => {}
                    IteratorStep::Done => return Ok(Value::Bool(true)),
                    IteratorStep::Abrupt(completion) => return completion.into_result(),
                }
            }
        }
    }

    fn close_set_predicate_iterator(
        &mut self,
        source: &mut crate::runtime::abstract_operations::IteratorSource,
        result: bool,
    ) -> Result<Value> {
        self.iterator_close(
            source,
            crate::runtime::control::Completion::Normal(Value::Bool(result)),
        )?
        .into_result()
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
        let raw_size = self.get_named(other, SET_SIZE_PROPERTY)?;
        let size = self.to_number(&raw_size)?;
        if size.is_nan() {
            return Err(Error::type_error(SET_LIKE_SIZE_NAN_ERROR));
        }
        let size = integer_or_infinity_from_number(size);
        if size < 0.0 {
            return Err(Error::exception(
                ErrorName::RangeError,
                SET_LIKE_SIZE_NEGATIVE_ERROR,
            ));
        }
        let has = self
            .get_named_method(other, SET_HAS_PROPERTY)?
            .ok_or_else(|| Error::type_error("Set-like argument has method is missing"))?;
        let keys = self
            .get_named_method(other, SET_KEYS_PROPERTY)?
            .ok_or_else(|| Error::type_error("Set-like argument keys method is missing"))?;
        Ok(SetRecord {
            object: other.clone(),
            size,
            has,
            keys,
        })
    }

    fn set_record_has(&mut self, record: &SetRecord, value: &Value) -> Result<bool> {
        let args = [value.clone()];
        let result = self.call_value(&record.has, &args, record.object.clone())?;
        to_boolean(self, &result)
    }

    /// Drive the iterator returned by the set-like `keys` method to completion.
    fn set_record_keys(&mut self, record: &SetRecord) -> Result<Vec<Value>> {
        let mut source = self.get_iterator_from_method(&record.object, &record.keys)?;
        let mut values = Vec::new();
        loop {
            self.step()?;
            match self.iterator_step(&mut source)? {
                IteratorStep::Value(value) => values.push(value),
                IteratorStep::Done => return Ok(values),
                IteratorStep::Abrupt(completion) => {
                    return completion.into_result().map(|_| values);
                }
            }
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
        let collection = self.create_collection(CollectionKind::Set)?;
        self.bind_collection_object(*object_id, CollectionKind::Set, collection)?;
        Ok((object, collection))
    }
}
