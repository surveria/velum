use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::IteratorStep,
        call::RuntimeCallArgs,
        object::{
            DataPropertyDescriptor, DataPropertyUpdate, OwnPropertyDescriptor,
            PropertyConfigurable, PropertyEnumerable, PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
        roots::VmRootKind,
    },
    value::Value,
};

const ARRAY_FROM_INDEX_LIMIT_ERROR: &str = "Array.from index exceeded supported range";
const ARRAY_FROM_PROPERTY_ERROR: &str = "Array.from could not define a result property";
const ARRAY_FROM_MAP_ERROR: &str = "Array.from map function is not callable";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

impl Context {
    pub(in crate::runtime::native) fn eval_array_from(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let args = args.as_slice();
        let items = args.first().cloned().unwrap_or(Value::Undefined);
        let map_function = args.get(1).cloned().unwrap_or(Value::Undefined);
        let this_argument = args.get(2).cloned().unwrap_or(Value::Undefined);
        let mapping = !matches!(map_function, Value::Undefined);
        if mapping && !self.semantic_is_callable(&map_function)? {
            return Err(Error::type_error(ARRAY_FROM_MAP_ERROR));
        }
        let _input_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            [&items, &map_function, &this_argument],
        )?;
        let iterator_method = self.flattenable_iterator_method(&items)?;
        let map_function = mapping.then_some(&map_function);
        if let Some(iterator_method) = iterator_method {
            return self.array_from_iterable(
                this_value,
                &items,
                &iterator_method,
                map_function,
                &this_argument,
            );
        }
        self.array_from_array_like(this_value, &items, map_function, &this_argument)
    }

    fn array_from_iterable(
        &mut self,
        constructor: &Value,
        items: &Value,
        iterator_method: &Value,
        map_function: Option<&Value>,
        this_argument: &Value,
    ) -> Result<Value> {
        let result = self.array_from_result(constructor, None)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, [iterator_method, &result])?;
        let mut iterator =
            self.get_iterator_from_method_with_array_fast_path(items, iterator_method)?;
        let _iterator_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, iterator.root_values())?;
        let mut index = 0_usize;
        loop {
            self.step()?;
            let step = match self.iterator_step(&mut iterator) {
                Ok(step) => step,
                Err(error) => {
                    return Err(self.iterator_close_on_error(&mut iterator, error));
                }
            };
            let value = match step {
                IteratorStep::Value(value) => value,
                IteratorStep::Done => {
                    self.set_array_like_length(&result, index)?;
                    return Ok(result);
                }
                IteratorStep::Abrupt(completion) => return completion.into_result(),
            };
            if u64::try_from(index).map_or(true, |value| value >= MAX_SAFE_INTEGER) {
                let error = Error::type_error(ARRAY_FROM_INDEX_LIMIT_ERROR);
                return Err(self.iterator_close_on_error(&mut iterator, error));
            }
            let mapped = match self.array_from_map_value(value, index, map_function, this_argument)
            {
                Ok(mapped) => mapped,
                Err(error) => {
                    return Err(self.iterator_close_on_error(&mut iterator, error));
                }
            };
            if let Err(error) = self.array_from_create_data_property(&result, index, mapped) {
                return Err(self.iterator_close_on_error(&mut iterator, error));
            }
            index = index
                .checked_add(1)
                .ok_or_else(|| Error::limit(ARRAY_FROM_INDEX_LIMIT_ERROR))?;
        }
    }

    fn array_from_array_like(
        &mut self,
        constructor: &Value,
        items: &Value,
        map_function: Option<&Value>,
        this_argument: &Value,
    ) -> Result<Value> {
        let length = self.array_like_length(items)?;
        let result = self.array_from_result(constructor, Some(length))?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        for index in 0..length {
            self.step()?;
            let value = self.get_array_like_index(items, index)?;
            let mapped = self.array_from_map_value(value, index, map_function, this_argument)?;
            self.array_from_create_data_property(&result, index, mapped)?;
        }
        self.set_array_like_length(&result, length)?;
        Ok(result)
    }

    fn array_from_result(&mut self, constructor: &Value, length: Option<usize>) -> Result<Value> {
        let length_value = length.map(Self::array_like_length_value).transpose()?;
        if self.semantic_is_constructor(constructor)? {
            let arguments = length_value.as_slice();
            return self.semantic_construct(constructor, arguments, constructor.clone());
        }
        let arguments = length_value.as_slice();
        self.eval_direct_array_constructor(arguments)
    }

    fn array_from_map_value(
        &mut self,
        value: Value,
        index: usize,
        map_function: Option<&Value>,
        this_argument: &Value,
    ) -> Result<Value> {
        let Some(map_function) = map_function else {
            return Ok(value);
        };
        let index = Self::array_like_index_value(index)?;
        self.call_value(map_function, &[value, index], this_argument.clone())
    }

    fn array_from_create_data_property(
        &mut self,
        result: &Value,
        index: usize,
        value: Value,
    ) -> Result<()> {
        let _value_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, [result, &value])?;
        let descriptor = DataPropertyDescriptor::new(
            value.clone(),
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        );
        let descriptor_value =
            self.create_property_descriptor_object(&OwnPropertyDescriptor::Data(descriptor))?;
        let _descriptor_scope = self.transient_root_scope(
            VmRootKind::TransientTemporary,
            std::iter::once(&descriptor_value),
        )?;
        let update = PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(value),
            Some(PropertyWritable::Yes),
            Some(PropertyEnumerable::Yes),
            Some(PropertyConfigurable::Yes),
        ));
        let mut property = DynamicPropertyKey::new(index.to_string(), None);
        if !self.semantic_define_own_property_update_with_descriptor(
            result,
            &mut property,
            update,
            &descriptor_value,
        )? {
            return Err(Error::type_error(ARRAY_FROM_PROPERTY_ERROR));
        }
        Ok(())
    }
}
