use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        collection_array_iterator::ArrayIterationTarget,
        collections::CollectionIteratorState,
        native::NativeFunctionKind,
        object::{
            DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable,
            PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
    },
    value::Value,
};

const ARRAY_ITERATOR_RECEIVER_ERROR: &str = "Array Iterator next requires an Array Iterator";
const ARRAY_ITERATOR_TAG: &str = "Array Iterator";
const ARRAY_ITERATOR_NEXT_PROPERTY: &str = "next";
const ARRAY_ITERATOR_PROTOTYPE_STATE_PROPERTY: &str = "\0ArrayIteratorPrototype";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";

impl Context {
    pub(in crate::runtime::native) fn eval_array_keys(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        self.create_array_iterator(this_value, ArrayIterationTarget::Keys)
    }

    pub(in crate::runtime::native) fn eval_array_values(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        self.create_array_iterator(this_value, ArrayIterationTarget::Values)
    }

    pub(in crate::runtime::native) fn eval_array_entries(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        self.create_array_iterator(this_value, ArrayIterationTarget::Entries)
    }

    fn create_array_iterator(
        &mut self,
        this_value: &Value,
        target: ArrayIterationTarget,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let prototype = self.array_iterator_prototype_id()?;
        let state = self.create_live_array_iterator(this_value.clone(), target)?;
        let state_value = self.create_ephemeral_native_function(
            NativeFunctionKind::CollectionIteratorNext(state),
            Value::Undefined,
        )?;
        let constructor_key = self.object_constructor_property_key()?;
        let object = self.objects.create_with_prototype(
            Some(prototype),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(object) = object else {
            return Err(Error::runtime("array iterator object creation failed"));
        };
        self.define_collection_iterator_state(object, state_value)?;
        Ok(Value::Object(object))
    }

    fn array_iterator_prototype_id(&mut self) -> Result<crate::value::ObjectId> {
        let array_prototype = self.array_constructor_prototype()?;
        let key = self.intern_property_key(ARRAY_ITERATOR_PROTOTYPE_STATE_PROPERTY)?;
        let property = DynamicPropertyKey::new(
            ARRAY_ITERATOR_PROTOTYPE_STATE_PROPERTY.to_owned(),
            Some(key),
        );
        let holder = Value::Object(array_prototype);
        if let Some(OwnPropertyDescriptor::Data(descriptor)) =
            self.semantic_own_property_descriptor(&holder, &property)?
        {
            let Value::Object(prototype) = descriptor.value() else {
                return Err(Error::runtime("Array Iterator prototype anchor is invalid"));
            };
            return Ok(prototype);
        }
        let parent = self.iterator_prototype_object_id()?;
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype(
            Some(parent),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let Value::Object(prototype) = prototype else {
            return Err(Error::runtime("Array Iterator prototype creation failed"));
        };
        let next =
            self.create_native_function(NativeFunctionKind::ArrayIteratorNext, Value::Undefined)?;
        let next_key = self.intern_property_key(ARRAY_ITERATOR_NEXT_PROPERTY)?;
        self.objects.define_property(
            prototype,
            next_key,
            ARRAY_ITERATOR_NEXT_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(next),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        self.install_array_iterator_tag(prototype)?;
        self.objects.define_property(
            array_prototype,
            key,
            ARRAY_ITERATOR_PROTOTYPE_STATE_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Object(prototype)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )?;
        Ok(prototype)
    }

    pub(in crate::runtime) fn default_array_iterator_next_is_intact(&mut self) -> Result<bool> {
        let prototype = self.array_iterator_prototype_id()?;
        let key = self.intern_property_key(ARRAY_ITERATOR_NEXT_PROPERTY)?;
        let property = DynamicPropertyKey::new(ARRAY_ITERATOR_NEXT_PROPERTY.to_owned(), Some(key));
        let holder = Value::Object(prototype);
        let Some(OwnPropertyDescriptor::Data(descriptor)) =
            self.semantic_own_property_descriptor(&holder, &property)?
        else {
            return Ok(false);
        };
        let Value::NativeFunction(id) = descriptor.value() else {
            return Ok(false);
        };
        Ok(self.native_function(id)?.kind() == NativeFunctionKind::ArrayIteratorNext)
    }

    fn install_array_iterator_tag(&mut self, prototype: crate::value::ObjectId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(tag) = tag else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(ARRAY_ITERATOR_TAG)?;
        self.objects.define_property(
            prototype,
            crate::runtime::object::PropertyKey::symbol(tag.id()),
            TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn eval_array_iterator_next(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let id = self.collection_iterator_receiver_state(this_value)?;
        let (owner, target, index, done) = match self.collection_iterators.get(id.index()) {
            Some(CollectionIteratorState::LiveArray(state)) => {
                (state.owner.clone(), state.target, state.cursor, state.done)
            }
            _ => return Err(Error::type_error(ARRAY_ITERATOR_RECEIVER_ERROR)),
        };
        if done {
            return self.create_iterator_result_object(Value::Undefined, true);
        }
        let length = self.array_like_length(&owner)?;
        if index >= length {
            let Some(CollectionIteratorState::LiveArray(state)) =
                self.collection_iterators.get_mut(id.index())
            else {
                return Err(Error::type_error(ARRAY_ITERATOR_RECEIVER_ERROR));
            };
            state.done = true;
            state.owner = Value::Undefined;
            return self.create_iterator_result_object(Value::Undefined, true);
        }
        let next = index
            .checked_add(1)
            .ok_or_else(|| Error::limit("array iterator cursor overflowed"))?;
        let Some(CollectionIteratorState::LiveArray(state)) =
            self.collection_iterators.get_mut(id.index())
        else {
            return Err(Error::type_error(ARRAY_ITERATOR_RECEIVER_ERROR));
        };
        state.cursor = next;
        let value = match target {
            ArrayIterationTarget::Keys => Self::array_like_index_value(index)?,
            ArrayIterationTarget::Values => self.get_array_like_index(&owner, index)?,
            ArrayIterationTarget::Entries => {
                let key = Self::array_like_index_value(index)?;
                let value = self.get_array_like_index(&owner, index)?;
                self.create_array_from_elements(vec![key, value])?
            }
        };
        self.create_iterator_result_object(value, false)
    }
}
