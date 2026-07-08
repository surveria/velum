use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        array_iterators::{ArrayIterationTarget, ArrayIteratorId},
        call::RuntimeCallArgs,
        object::{
            DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable,
            PropertyKey, PropertyUpdate, PropertyWritable,
        },
    },
    value::Value,
};

use super::NativeFunctionKind;

const ARRAY_ITERATOR_RECEIVER_ERROR: &str = "Array iterator method requires an object receiver";
const ITERATOR_NEXT_NAME: &str = "next";
const ITERATOR_RESULT_DONE_NAME: &str = "done";
const ITERATOR_RESULT_VALUE_NAME: &str = "value";
const ITERATOR_SYMBOL_DISPLAY: &str = "[Symbol.iterator]";
const LENGTH_PROPERTY: &str = "length";

impl Context {
    pub(in crate::runtime::native) fn eval_array_keys(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_keys(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_keys(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::eval_array_discard_args(args);
        self.create_array_iterator_object(this_value.clone(), ArrayIterationTarget::Keys)
    }

    pub(in crate::runtime::native) fn eval_array_values(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_values(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_values(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::eval_array_discard_args(args);
        self.create_array_iterator_object(this_value.clone(), ArrayIterationTarget::Values)
    }

    pub(in crate::runtime::native) fn eval_array_entries(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_entries(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_entries(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::eval_array_discard_args(args);
        self.create_array_iterator_object(this_value.clone(), ArrayIterationTarget::Entries)
    }

    fn create_array_iterator_object(
        &mut self,
        source: Value,
        target: ArrayIterationTarget,
    ) -> Result<Value> {
        Self::ensure_array_iterator_source(&source)?;
        let iterator_id = self.create_array_iterator(source, target)?;
        let next = self.create_native_function(
            NativeFunctionKind::ArrayIteratorNext(iterator_id),
            Value::Undefined,
        )?;
        let next_key = self.intern_property_key(ITERATOR_NEXT_NAME)?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object_id) = &object else {
            return Err(Error::runtime("Array iterator object creation failed"));
        };
        self.objects.define_property(
            *object_id,
            next_key,
            ITERATOR_NEXT_NAME,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(next),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.install_array_iterator_self_method(*object_id)?;
        Ok(object)
    }

    fn install_array_iterator_self_method(
        &mut self,
        object_id: crate::value::ObjectId,
    ) -> Result<()> {
        self.symbol_constructor_value()?;
        let Some(symbol) = self.iterator_symbol() else {
            return Err(Error::runtime("Symbol.iterator is not initialized"));
        };
        let self_fn =
            self.create_native_function(NativeFunctionKind::IteratorSelf, Value::Undefined)?;
        self.objects.define_property(
            object_id,
            PropertyKey::symbol(symbol),
            ITERATOR_SYMBOL_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(self_fn),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn eval_array_iterator_next(
        &mut self,
        iterator: ArrayIteratorId,
    ) -> Result<Value> {
        let (source, index, target) = self.array_iterator_snapshot(iterator)?;
        let length = self.array_iterator_length(&source)?;
        if index >= length {
            return self.array_iterator_result(Value::Undefined, true);
        }
        let value = self.array_iterator_value(&source, index, target)?;
        self.advance_array_iterator(iterator)?;
        self.array_iterator_result(value, false)
    }

    fn array_iterator_value(
        &mut self,
        source: &Value,
        index: usize,
        target: ArrayIterationTarget,
    ) -> Result<Value> {
        match target {
            ArrayIterationTarget::Keys => Self::array_like_index_value(index),
            ArrayIterationTarget::Values => self.get_array_like_index(source, index),
            ArrayIterationTarget::Entries => {
                let key = Self::array_like_index_value(index)?;
                let value = self.get_array_like_index(source, index)?;
                self.create_array_from_elements(vec![key, value])
            }
        }
    }

    fn array_iterator_length(&mut self, source: &Value) -> Result<usize> {
        let length = self.get_property_value(source, LENGTH_PROPERTY)?;
        Self::array_to_length_value(&length)
    }

    fn array_iterator_result(&mut self, value: Value, done: bool) -> Result<Value> {
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

    fn ensure_array_iterator_source(source: &Value) -> Result<()> {
        if matches!(
            source,
            Value::Object(_) | Value::String(_) | Value::HeapString(_)
        ) {
            return Ok(());
        }
        Err(Error::type_error(ARRAY_ITERATOR_RECEIVER_ERROR))
    }
}
