use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{IteratorResultStep, IteratorSource, IteratorStep},
        call::RuntimeCallArgs,
        collections::{
            CollectionIteratorId, IteratorConcatInput, IteratorRecordState, IteratorStaticKind,
            IteratorStaticState, IteratorZipMode,
        },
        control::Completion,
        native::{
            ITERATOR_CONCAT_NAME, ITERATOR_ZIP_KEYED_NAME, ITERATOR_ZIP_NAME, IteratorFunctionKind,
            NativeFunctionKind,
        },
        object::{
            DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable,
            PropertyUpdate, PropertyWritable,
        },
        roots::VmRootKind,
        transient_roots::TransientRootScope,
    },
    value::{NativeFunctionId, Value},
};

const ITERATOR_NEXT_NAME: &str = "next";
const ITERATOR_RETURN_NAME: &str = "return";
const ZIP_MODE_NAME: &str = "mode";
const ZIP_PADDING_NAME: &str = "padding";
const ZIP_SHORTEST_NAME: &str = "shortest";
const ZIP_LONGEST_NAME: &str = "longest";
const ZIP_STRICT_NAME: &str = "strict";
const STATIC_ITERATOR_RUNNING_ERROR: &str = "Iterator helper is already running";
const STATIC_ITERATOR_INPUT_ERROR: &str = "Iterator combinator input must be an object";
const STATIC_ITERATOR_OPTIONS_ERROR: &str = "Iterator zip options must be an object";
const STATIC_ITERATOR_MODE_ERROR: &str = "Iterator zip mode is invalid";
const STATIC_ITERATOR_PADDING_ERROR: &str = "Iterator zip padding must be an object";
const STATIC_ITERATOR_LENGTH_ERROR: &str = "Iterator combinator count overflowed";

impl Context {
    pub(in crate::runtime::native) fn install_iterator_static_methods(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        let methods = [
            (ITERATOR_CONCAT_NAME, IteratorFunctionKind::Concat),
            (ITERATOR_ZIP_NAME, IteratorFunctionKind::Zip),
            (ITERATOR_ZIP_KEYED_NAME, IteratorFunctionKind::ZipKeyed),
        ];
        for (name, kind) in methods {
            let method =
                self.create_native_function(NativeFunctionKind::Iterator(kind), Value::Undefined)?;
            let key = self.intern_property_key(name)?;
            self.define_native_function_property_key(
                constructor,
                name,
                key,
                DataPropertyUpdate::new(
                    Some(method),
                    Some(PropertyWritable::Yes),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                ),
            )?;
        }
        Ok(())
    }

    pub(in crate::runtime::native) fn eval_iterator_concat(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        let mut inputs = Vec::with_capacity(args.as_slice().len());
        for item in args.as_slice() {
            self.require_static_iterator_object(item)?;
            let Some(open_method) = self.flattenable_iterator_method(item)? else {
                return Err(Error::type_error(STATIC_ITERATOR_INPUT_ERROR));
            };
            roots.add_values([item, &open_method])?;
            inputs.push(IteratorConcatInput {
                iterable: item.clone(),
                open_method,
            });
        }
        self.create_static_iterator_object(IteratorStaticKind::Concat {
            inputs,
            index: 0,
            active: None,
        })
    }

    pub(in crate::runtime::native) fn eval_iterator_zip(
        &mut self,
        args: RuntimeCallArgs<'_>,
        keyed: bool,
    ) -> Result<Value> {
        let iterables = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        self.require_static_iterator_object(&iterables)?;
        let (mode, padding_option) = self.iterator_zip_options(&args)?;
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        let (records, keys) = if keyed {
            self.iterator_zip_keyed_records(&iterables, &roots)?
        } else {
            (self.iterator_zip_records(&iterables, &roots)?, None)
        };
        let padding = self.iterator_zip_padding(mode, padding_option, keys.as_deref(), &records)?;
        roots.add_values(padding.iter())?;
        self.create_static_iterator_object(IteratorStaticKind::Zip {
            records: records.into_iter().map(Some).collect(),
            mode,
            padding,
            keys,
        })
    }

    fn require_static_iterator_object(&self, value: &Value) -> Result<()> {
        if self.semantic_object_ref(value)?.is_none() {
            return Err(Error::type_error(STATIC_ITERATOR_INPUT_ERROR));
        }
        Ok(())
    }

    fn iterator_zip_options(
        &mut self,
        args: &RuntimeCallArgs<'_>,
    ) -> Result<(IteratorZipMode, Option<Value>)> {
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options, Value::Undefined) {
            return Ok((IteratorZipMode::Shortest, None));
        }
        if self.semantic_object_ref(&options)?.is_none() {
            return Err(Error::type_error(STATIC_ITERATOR_OPTIONS_ERROR));
        }
        let mode_value = self.get_named(&options, ZIP_MODE_NAME)?;
        let mode = match mode_value.string_text() {
            Some(ZIP_SHORTEST_NAME) => IteratorZipMode::Shortest,
            Some(ZIP_LONGEST_NAME) => IteratorZipMode::Longest,
            Some(ZIP_STRICT_NAME) => IteratorZipMode::Strict,
            None if matches!(mode_value, Value::Undefined) => IteratorZipMode::Shortest,
            Some(_) | None => return Err(Error::type_error(STATIC_ITERATOR_MODE_ERROR)),
        };
        if mode != IteratorZipMode::Longest {
            return Ok((mode, None));
        }
        let padding = self.get_named(&options, ZIP_PADDING_NAME)?;
        if matches!(padding, Value::Undefined) {
            return Ok((mode, None));
        }
        if self.semantic_object_ref(&padding)?.is_none() {
            return Err(Error::type_error(STATIC_ITERATOR_PADDING_ERROR));
        }
        Ok((mode, Some(padding)))
    }

    fn iterator_zip_records(
        &mut self,
        iterables: &Value,
        roots: &TransientRootScope,
    ) -> Result<Vec<IteratorRecordState>> {
        let mut input = self.get_iterator(iterables)?;
        let mut records = Vec::new();
        loop {
            self.step()?;
            let value = match self.iterator_step(&mut input) {
                Ok(IteratorStep::Value(value)) => value,
                Ok(IteratorStep::Done) => return Ok(records),
                Ok(IteratorStep::Abrupt(completion)) => {
                    return Err(self.close_records_on_completion(records, completion));
                }
                Err(error) => return Err(self.close_records_on_error(records, error)),
            };
            let record = match self.iterator_record_from_flattenable(&value) {
                Ok(record) => record,
                Err(error) => {
                    let error = self.close_records_on_error(records, error);
                    return Err(self.close_source_on_error(&mut input, error));
                }
            };
            roots.add_values([&record.iterator, &record.next])?;
            records.push(record);
        }
    }

    fn iterator_zip_keyed_records(
        &mut self,
        iterables: &Value,
        roots: &TransientRootScope,
    ) -> Result<(Vec<IteratorRecordState>, Option<Vec<Value>>)> {
        let source_keys = self.semantic_own_property_keys(iterables)?;
        roots.add_values(source_keys.iter())?;
        let mut records = Vec::new();
        let mut keys = Vec::new();
        for key in source_keys {
            self.step()?;
            let property = self.dynamic_property_key(&key)?;
            let descriptor = match self.semantic_own_property_descriptor(iterables, &property) {
                Ok(descriptor) => descriptor,
                Err(error) => return Err(self.close_records_on_error(records, error)),
            };
            let Some(descriptor) = descriptor else {
                continue;
            };
            let enumerable = match descriptor {
                OwnPropertyDescriptor::Data(descriptor) => descriptor.enumerable(),
                OwnPropertyDescriptor::Accessor(descriptor) => descriptor.enumerable(),
            };
            if !enumerable.is_yes() {
                continue;
            }
            let value = match self.get(iterables, property.lookup()) {
                Ok(value) => value,
                Err(error) => return Err(self.close_records_on_error(records, error)),
            };
            if matches!(value, Value::Undefined) {
                continue;
            }
            let record = match self.iterator_record_from_flattenable(&value) {
                Ok(record) => record,
                Err(error) => return Err(self.close_records_on_error(records, error)),
            };
            roots.add_values([&key, &record.iterator, &record.next])?;
            keys.push(key);
            records.push(record);
        }
        Ok((records, Some(keys)))
    }

    fn iterator_record_from_flattenable(&mut self, value: &Value) -> Result<IteratorRecordState> {
        self.require_static_iterator_object(value)?;
        let method = self.flattenable_iterator_method(value)?;
        let iterator = if let Some(method) = method {
            self.call_value(&method, &[], value.clone())?
        } else if let Value::Object(id) = value {
            if let Some(units) = self.string_object_utf16_primitive_value(*id)? {
                let units = units.to_vec();
                self.static_string_object_iterator(&units)?
            } else {
                value.clone()
            }
        } else {
            value.clone()
        };
        self.require_static_iterator_object(&iterator)?;
        let scope = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        scope.add_values(std::iter::once(&iterator))?;
        let next = self.iterator_direct_next(&iterator)?;
        Ok(IteratorRecordState { iterator, next })
    }

    fn static_string_object_iterator(&mut self, units: &[u16]) -> Result<Value> {
        let mut items = Vec::new();
        let mut index = 0_usize;
        while let Some(first) = units.get(index).copied() {
            let mut code_point = vec![first];
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::limit(STATIC_ITERATOR_LENGTH_ERROR))?;
            if (0xD800..=0xDBFF).contains(&first)
                && units
                    .get(index)
                    .is_some_and(|second| (0xDC00..=0xDFFF).contains(second))
            {
                let Some(second) = units.get(index).copied() else {
                    return Err(Error::runtime("string iterator lookahead disappeared"));
                };
                code_point.push(second);
                index = index
                    .checked_add(1)
                    .ok_or_else(|| Error::limit(STATIC_ITERATOR_LENGTH_ERROR))?;
            }
            items.push(self.heap_utf16_string_value(&code_point)?);
        }
        self.create_collection_iterator_object(items)
    }

    fn iterator_zip_padding(
        &mut self,
        mode: IteratorZipMode,
        padding_option: Option<Value>,
        keys: Option<&[Value]>,
        records: &[IteratorRecordState],
    ) -> Result<Vec<Value>> {
        if mode != IteratorZipMode::Longest {
            return Ok(Vec::new());
        }
        let Some(padding_option) = padding_option else {
            return Ok(vec![Value::Undefined; records.len()]);
        };
        if let Some(keys) = keys {
            let mut padding = Vec::with_capacity(keys.len());
            for key in keys {
                let property = self.dynamic_property_key(key)?;
                match self.get(&padding_option, property.lookup()) {
                    Ok(value) => padding.push(value),
                    Err(error) => return Err(self.close_records_on_error(records.to_vec(), error)),
                }
            }
            return Ok(padding);
        }
        let mut source = match self.get_iterator(&padding_option) {
            Ok(source) => source,
            Err(error) => return Err(self.close_records_on_error(records.to_vec(), error)),
        };
        let mut padding = Vec::with_capacity(records.len());
        let mut exhausted = false;
        for _ in records {
            if exhausted {
                padding.push(Value::Undefined);
                continue;
            }
            match self.iterator_step(&mut source) {
                Ok(IteratorStep::Value(value)) => padding.push(value),
                Ok(IteratorStep::Done) => {
                    exhausted = true;
                    padding.push(Value::Undefined);
                }
                Ok(IteratorStep::Abrupt(completion)) => {
                    return Err(self.close_records_on_completion(records.to_vec(), completion));
                }
                Err(error) => {
                    return Err(self.close_records_on_error(records.to_vec(), error));
                }
            }
        }
        if !exhausted {
            let completion =
                match self.iterator_close(&mut source, Completion::Normal(Value::Undefined)) {
                    Ok(completion) => completion,
                    Err(error) => return Err(self.close_records_on_error(records.to_vec(), error)),
                };
            if let Err(error) = completion.into_native_value_result() {
                return Err(self.close_records_on_error(records.to_vec(), error));
            }
        }
        Ok(padding)
    }

    fn create_static_iterator_object(&mut self, kind: IteratorStaticKind) -> Result<Value> {
        let prototype = self.iterator_helper_prototype_id()?;
        let id = self.create_static_iterator(IteratorStaticState {
            started: false,
            running: false,
            done: false,
            kind,
        })?;
        let next = self.create_native_function(
            NativeFunctionKind::Iterator(IteratorFunctionKind::StaticNext(id)),
            Value::Undefined,
        )?;
        let return_fn = self.create_native_function(
            NativeFunctionKind::Iterator(IteratorFunctionKind::StaticReturn(id)),
            Value::Undefined,
        )?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = object else {
            return Err(Error::runtime("static iterator object creation failed"));
        };
        self.define_non_enumerable_object_property(object_id, ITERATOR_NEXT_NAME, next)?;
        self.define_non_enumerable_object_property(object_id, ITERATOR_RETURN_NAME, return_fn)?;
        Ok(Value::Object(object_id))
    }

    pub(in crate::runtime::native) fn eval_iterator_static_next(
        &mut self,
        id: CollectionIteratorId,
    ) -> Result<Value> {
        let is_concat = {
            let state = self.iterator_static_state_mut(id)?;
            if state.running {
                return Err(Error::type_error(STATIC_ITERATOR_RUNNING_ERROR));
            }
            if state.done {
                return self.create_iterator_result_object(Value::Undefined, true);
            }
            state.started = true;
            state.running = true;
            matches!(state.kind, IteratorStaticKind::Concat { .. })
        };
        let result = if is_concat {
            self.iterator_concat_next(id)
        } else {
            self.iterator_zip_next(id)
        };
        let state = self.iterator_static_state_mut(id)?;
        state.running = false;
        if result.is_err() {
            state.done = true;
        }
        result
    }

    fn iterator_concat_next(&mut self, id: CollectionIteratorId) -> Result<Value> {
        loop {
            self.step()?;
            let active = {
                let state = self.iterator_static_state(id)?;
                let IteratorStaticKind::Concat { active, .. } = &state.kind else {
                    return Err(Error::runtime("static iterator kind changed"));
                };
                active.clone()
            };
            let record = if let Some(active) = active {
                active
            } else {
                let input = {
                    let state = self.iterator_static_state_mut(id)?;
                    let IteratorStaticKind::Concat { inputs, index, .. } = &mut state.kind else {
                        return Err(Error::runtime("static iterator kind changed"));
                    };
                    let Some(input) = inputs.get(*index).cloned() else {
                        state.done = true;
                        return self.create_iterator_result_object(Value::Undefined, true);
                    };
                    *index = index
                        .checked_add(1)
                        .ok_or_else(|| Error::limit(STATIC_ITERATOR_LENGTH_ERROR))?;
                    input
                };
                let source = self.get_iterator_from_method(&input.iterable, &input.open_method)?;
                let record = Self::protocol_record(source)?;
                let state = self.iterator_static_state_mut(id)?;
                let IteratorStaticKind::Concat { active, .. } = &mut state.kind else {
                    return Err(Error::runtime("static iterator kind changed"));
                };
                *active = Some(record.clone());
                record
            };
            let mut source = Self::record_source(record);
            match self.iterator_step(&mut source)? {
                IteratorStep::Value(value) => {
                    return self.create_iterator_result_object(value, false);
                }
                IteratorStep::Done => {
                    let state = self.iterator_static_state_mut(id)?;
                    let IteratorStaticKind::Concat { active, .. } = &mut state.kind else {
                        return Err(Error::runtime("static iterator kind changed"));
                    };
                    *active = None;
                }
                IteratorStep::Abrupt(completion) => return completion.into_result(),
            }
        }
    }

    fn iterator_zip_next(&mut self, id: CollectionIteratorId) -> Result<Value> {
        let (count, mode) = {
            let state = self.iterator_static_state(id)?;
            let IteratorStaticKind::Zip { records, mode, .. } = &state.kind else {
                return Err(Error::runtime("static iterator kind changed"));
            };
            (records.len(), *mode)
        };
        if count == 0 {
            self.iterator_static_state_mut(id)?.done = true;
            return self.create_iterator_result_object(Value::Undefined, true);
        }
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        let mut results = Vec::with_capacity(count);
        for index in 0..count {
            let record = self.zip_record(id, index)?;
            let Some(record) = record else {
                let padding = self.zip_padding_value(id, index)?;
                roots.add_values(std::iter::once(&padding))?;
                results.push(padding);
                continue;
            };
            let mut source = Self::record_source(record);
            match self.iterator_step(&mut source) {
                Ok(IteratorStep::Value(value)) => {
                    roots.add_values(std::iter::once(&value))?;
                    results.push(value);
                }
                Ok(IteratorStep::Done) => {
                    self.clear_zip_record(id, index)?;
                    if let Some(result) = self.finish_zip_done(id, index, mode, &roots)? {
                        return Ok(result);
                    }
                    let padding = self.zip_padding_value(id, index)?;
                    roots.add_values(std::iter::once(&padding))?;
                    results.push(padding);
                }
                Ok(IteratorStep::Abrupt(completion)) => {
                    self.clear_zip_record(id, index)?;
                    return Err(self.close_zip_on_completion(id, completion));
                }
                Err(error) => {
                    self.clear_zip_record(id, index)?;
                    return Err(self.close_zip_on_error(id, error));
                }
            }
        }
        let value = self.finish_zip_results(id, results)?;
        self.create_iterator_result_object(value, false)
    }

    fn finish_zip_done(
        &mut self,
        id: CollectionIteratorId,
        index: usize,
        mode: IteratorZipMode,
        roots: &TransientRootScope,
    ) -> Result<Option<Value>> {
        match mode {
            IteratorZipMode::Shortest => {
                self.complete_zip_normally(id)?;
                Ok(Some(
                    self.create_iterator_result_object(Value::Undefined, true)?,
                ))
            }
            IteratorZipMode::Longest => {
                if self.zip_has_open_records(id)? {
                    Ok(None)
                } else {
                    self.iterator_static_state_mut(id)?.done = true;
                    Ok(Some(
                        self.create_iterator_result_object(Value::Undefined, true)?,
                    ))
                }
            }
            IteratorZipMode::Strict if index != 0 => {
                let error = Error::type_error("Iterator.zip strict inputs have different lengths");
                Err(self.close_zip_on_error(id, error))
            }
            IteratorZipMode::Strict => {
                let count = self.zip_record_count(id)?;
                for remaining in 1..count {
                    let Some(record) = self.zip_record(id, remaining)? else {
                        continue;
                    };
                    let mut source = Self::record_source(record);
                    match self.iterator_step_result(&mut source) {
                        Ok(IteratorResultStep::Done) => self.clear_zip_record(id, remaining)?,
                        Ok(IteratorResultStep::Result(result)) => {
                            roots.add_values(std::iter::once(&result))?;
                            let error = Error::type_error(
                                "Iterator.zip strict inputs have different lengths",
                            );
                            return Err(self.close_zip_on_error(id, error));
                        }
                        Ok(IteratorResultStep::Abrupt(completion)) => {
                            self.clear_zip_record(id, remaining)?;
                            return Err(self.close_zip_on_completion(id, completion));
                        }
                        Err(error) => {
                            self.clear_zip_record(id, remaining)?;
                            return Err(self.close_zip_on_error(id, error));
                        }
                    }
                }
                self.iterator_static_state_mut(id)?.done = true;
                Ok(Some(
                    self.create_iterator_result_object(Value::Undefined, true)?,
                ))
            }
        }
    }

    fn finish_zip_results(
        &mut self,
        id: CollectionIteratorId,
        results: Vec<Value>,
    ) -> Result<Value> {
        let keys = {
            let state = self.iterator_static_state(id)?;
            let IteratorStaticKind::Zip { keys, .. } = &state.kind else {
                return Err(Error::runtime("static iterator kind changed"));
            };
            keys.clone()
        };
        let Some(keys) = keys else {
            return self.create_array_from_elements(results);
        };
        let object = self
            .objects
            .create_with_exact_prototype(None, self.limits.max_objects)?;
        for (key, value) in keys.iter().zip(results) {
            let mut property = self.dynamic_property_key(key)?;
            let update = PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::Yes),
                Some(PropertyConfigurable::Yes),
            ));
            if !self.semantic_define_own_property_update(&object, &mut property, update)? {
                return Err(Error::type_error(
                    "Iterator.zipKeyed result property could not be defined",
                ));
            }
        }
        Ok(object)
    }

    pub(in crate::runtime::native) fn eval_iterator_static_return(
        &mut self,
        id: CollectionIteratorId,
    ) -> Result<Value> {
        let records = {
            let state = self.iterator_static_state_mut(id)?;
            if state.running {
                return Err(Error::type_error(STATIC_ITERATOR_RUNNING_ERROR));
            }
            if state.done {
                return self.create_iterator_result_object(Value::Undefined, true);
            }
            state.done = true;
            state.running = state.started;
            match &mut state.kind {
                IteratorStaticKind::Concat { active, .. } => active.take().into_iter().collect(),
                IteratorStaticKind::Zip { records, .. } => {
                    records.iter_mut().filter_map(Option::take).collect()
                }
            }
        };
        let result = self.close_records_normally(records);
        self.iterator_static_state_mut(id)?.running = false;
        result?;
        self.create_iterator_result_object(Value::Undefined, true)
    }

    fn zip_record(
        &self,
        id: CollectionIteratorId,
        index: usize,
    ) -> Result<Option<IteratorRecordState>> {
        let state = self.iterator_static_state(id)?;
        let IteratorStaticKind::Zip { records, .. } = &state.kind else {
            return Err(Error::runtime("static iterator kind changed"));
        };
        records
            .get(index)
            .cloned()
            .ok_or_else(|| Error::runtime("zip iterator index is out of range"))
    }

    fn clear_zip_record(&mut self, id: CollectionIteratorId, index: usize) -> Result<()> {
        let state = self.iterator_static_state_mut(id)?;
        let IteratorStaticKind::Zip { records, .. } = &mut state.kind else {
            return Err(Error::runtime("static iterator kind changed"));
        };
        let record = records
            .get_mut(index)
            .ok_or_else(|| Error::runtime("zip iterator index is out of range"))?;
        *record = None;
        Ok(())
    }

    fn zip_padding_value(&self, id: CollectionIteratorId, index: usize) -> Result<Value> {
        let state = self.iterator_static_state(id)?;
        let IteratorStaticKind::Zip { padding, .. } = &state.kind else {
            return Err(Error::runtime("static iterator kind changed"));
        };
        padding
            .get(index)
            .cloned()
            .ok_or_else(|| Error::runtime("zip padding index is out of range"))
    }

    fn zip_record_count(&self, id: CollectionIteratorId) -> Result<usize> {
        let state = self.iterator_static_state(id)?;
        let IteratorStaticKind::Zip { records, .. } = &state.kind else {
            return Err(Error::runtime("static iterator kind changed"));
        };
        Ok(records.len())
    }

    fn zip_has_open_records(&self, id: CollectionIteratorId) -> Result<bool> {
        let state = self.iterator_static_state(id)?;
        let IteratorStaticKind::Zip { records, .. } = &state.kind else {
            return Err(Error::runtime("static iterator kind changed"));
        };
        Ok(records.iter().any(Option::is_some))
    }

    fn take_zip_records(&mut self, id: CollectionIteratorId) -> Result<Vec<IteratorRecordState>> {
        let state = self.iterator_static_state_mut(id)?;
        state.done = true;
        let IteratorStaticKind::Zip { records, .. } = &mut state.kind else {
            return Err(Error::runtime("static iterator kind changed"));
        };
        Ok(records.iter_mut().filter_map(Option::take).collect())
    }

    fn complete_zip_normally(&mut self, id: CollectionIteratorId) -> Result<()> {
        let records = self.take_zip_records(id)?;
        self.close_records_normally(records)
    }

    fn close_zip_on_error(&mut self, id: CollectionIteratorId, error: Error) -> Error {
        match self.take_zip_records(id) {
            Ok(records) => self.close_records_on_error(records, error),
            Err(state_error) => state_error,
        }
    }

    fn close_zip_on_completion(
        &mut self,
        id: CollectionIteratorId,
        completion: Completion,
    ) -> Error {
        match self.take_zip_records(id) {
            Ok(records) => self.close_records_on_completion(records, completion),
            Err(state_error) => state_error,
        }
    }

    fn close_records_normally(&mut self, mut records: Vec<IteratorRecordState>) -> Result<()> {
        let mut completion = Completion::Normal(Value::Undefined);
        while let Some(record) = records.pop() {
            let mut source = Self::record_source(record);
            completion = match self.iterator_close(&mut source, completion) {
                Ok(completion) => completion,
                Err(error) => return Err(self.close_records_on_error(records, error)),
            };
        }
        completion.into_native_value_result().map(|_value| ())
    }

    fn close_records_on_completion(
        &mut self,
        records: Vec<IteratorRecordState>,
        completion: Completion,
    ) -> Error {
        match completion.into_result() {
            Ok(_value) => Error::runtime("abrupt iterator completion was normal"),
            Err(error) => self.close_records_on_error(records, error),
        }
    }

    fn close_records_on_error(
        &mut self,
        mut records: Vec<IteratorRecordState>,
        mut error: Error,
    ) -> Error {
        while let Some(record) = records.pop() {
            let mut source = Self::record_source(record);
            error = self.iterator_close_on_error(&mut source, error);
        }
        error
    }

    fn close_source_on_error(&mut self, source: &mut IteratorSource, error: Error) -> Error {
        self.iterator_close_on_error(source, error)
    }

    fn protocol_record(source: IteratorSource) -> Result<IteratorRecordState> {
        let IteratorSource::Protocol { iterator, next, .. } = source else {
            return Err(Error::runtime(
                "captured iterator source is not a protocol record",
            ));
        };
        Ok(IteratorRecordState { iterator, next })
    }

    fn record_source(record: IteratorRecordState) -> IteratorSource {
        IteratorSource::Protocol {
            iterator: record.iterator,
            next: record.next,
            done: false,
        }
    }
}
