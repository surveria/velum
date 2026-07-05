use crate::value::Value;

use super::{ARRAY_LENGTH_PROPERTY, ArrayIndex, Object, ObjectHeap, PropertyKey, PropertyLookup};
use crate::error::Result;
use crate::value::ObjectId;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PropertyEnumerable {
    Yes,
    No,
}

impl PropertyEnumerable {
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PropertyWritable {
    Yes,
    No,
}

impl PropertyWritable {
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PropertyConfigurable {
    Yes,
    No,
}

impl PropertyConfigurable {
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Debug, Clone)]
pub struct DataPropertyDescriptor {
    value: Value,
    writable: PropertyWritable,
    enumerable: PropertyEnumerable,
    configurable: PropertyConfigurable,
}

impl DataPropertyDescriptor {
    pub const fn new(
        value: Value,
        writable: PropertyWritable,
        enumerable: PropertyEnumerable,
        configurable: PropertyConfigurable,
    ) -> Self {
        Self {
            value,
            writable,
            enumerable,
            configurable,
        }
    }

    pub fn value(&self) -> Value {
        self.value.clone()
    }

    pub const fn writable(&self) -> PropertyWritable {
        self.writable
    }

    pub const fn enumerable(&self) -> PropertyEnumerable {
        self.enumerable
    }

    pub const fn configurable(&self) -> PropertyConfigurable {
        self.configurable
    }
}

#[derive(Debug, Clone)]
pub struct DataPropertyUpdate {
    value: Option<Value>,
    writable: Option<PropertyWritable>,
    enumerable: Option<PropertyEnumerable>,
    configurable: Option<PropertyConfigurable>,
}

impl DataPropertyUpdate {
    pub const fn new(
        value: Option<Value>,
        writable: Option<PropertyWritable>,
        enumerable: Option<PropertyEnumerable>,
        configurable: Option<PropertyConfigurable>,
    ) -> Self {
        Self {
            value,
            writable,
            enumerable,
            configurable,
        }
    }

    pub fn value(&self) -> Option<Value> {
        self.value.clone()
    }

    pub const fn writable(&self) -> Option<PropertyWritable> {
        self.writable
    }

    pub const fn enumerable(&self) -> Option<PropertyEnumerable> {
        self.enumerable
    }

    pub const fn configurable(&self) -> Option<PropertyConfigurable> {
        self.configurable
    }

    pub fn complete_for_new(self) -> DataPropertyDescriptor {
        DataPropertyDescriptor::new(
            self.value.unwrap_or(Value::Undefined),
            self.writable.unwrap_or(PropertyWritable::No),
            self.enumerable.unwrap_or(PropertyEnumerable::No),
            self.configurable.unwrap_or(PropertyConfigurable::No),
        )
    }
}

#[derive(Debug, Clone)]
pub struct ObjectProperty {
    descriptor: DataPropertyDescriptor,
}

impl ObjectProperty {
    pub const fn ordinary(value: Value, enumerable: PropertyEnumerable) -> Self {
        Self {
            descriptor: DataPropertyDescriptor::new(
                value,
                PropertyWritable::Yes,
                enumerable,
                PropertyConfigurable::Yes,
            ),
        }
    }

    const fn from_descriptor(descriptor: DataPropertyDescriptor) -> Self {
        Self { descriptor }
    }

    pub fn value(&self) -> Value {
        self.descriptor.value()
    }

    pub const fn is_enumerable(&self) -> bool {
        self.descriptor.enumerable().is_yes()
    }

    pub const fn is_configurable(&self) -> bool {
        self.descriptor.configurable().is_yes()
    }

    pub fn descriptor(&self) -> DataPropertyDescriptor {
        self.descriptor.clone()
    }

    pub fn set_value(&mut self, value: Value) {
        if self.descriptor.writable().is_yes() {
            self.descriptor.value = value;
        }
    }

    pub fn define(&mut self, update: DataPropertyUpdate) {
        if let Some(value) = update.value {
            self.descriptor.value = value;
        }
        if let Some(writable) = update.writable {
            self.descriptor.writable = writable;
        }
        if let Some(enumerable) = update.enumerable {
            self.descriptor.enumerable = enumerable;
        }
        if let Some(configurable) = update.configurable {
            self.descriptor.configurable = configurable;
        }
    }

    pub const fn set_enumerable(&mut self, enumerable: PropertyEnumerable) {
        self.descriptor.enumerable = enumerable;
    }
}

impl ObjectHeap {
    pub fn own_property_descriptor(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<DataPropertyDescriptor>> {
        self.object(id)
            .map(|object| object.own_property_descriptor(property))
    }

    pub fn define_property(
        &mut self,
        id: ObjectId,
        property: PropertyKey,
        property_name: &str,
        update: DataPropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        let object = self.object_mut(id)?;
        object.define_property(property, property_name, update, max_properties)
    }

    pub fn has_own(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        self.object(id).map(|object| object.has_own(property))
    }
}

impl Object {
    fn own_property_descriptor(
        &self,
        property: PropertyLookup<'_>,
    ) -> Option<DataPropertyDescriptor> {
        if let Some(length) = self
            .array_length
            .filter(|_| property.name() == ARRAY_LENGTH_PROPERTY)
        {
            return Some(DataPropertyDescriptor::new(
                length.value(),
                PropertyWritable::Yes,
                PropertyEnumerable::No,
                PropertyConfigurable::No,
            ));
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property.name())
            && let Some(descriptor) = self.array_element_descriptor(index)
        {
            return Some(descriptor);
        }
        let key = property.key()?;
        self.named_property(key).map(ObjectProperty::descriptor)
    }

    fn define_property(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        update: DataPropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        let index = ArrayIndex::parse(property_name);
        if self.array_length.is_some()
            && let Some(index) = index
        {
            return self.define_array_property(index, property, update, max_properties);
        }
        self.define_named_property(property, update, max_properties)?;
        if let Some(index) = index {
            self.sparse_array_keys.insert(index, property);
        }
        Ok(())
    }

    fn define_named_property(
        &mut self,
        property: PropertyKey,
        update: DataPropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        let property_count = self.property_count();
        let enumerable_update = if self.properties.contains_key(&property) {
            let existing = self.named_property_mut(property)?;
            let was_enumerable = existing.is_enumerable();
            existing.define(update);
            Some((was_enumerable, existing.is_enumerable()))
        } else {
            if property_count >= max_properties {
                return Err(crate::error::Error::limit(format!(
                    "object property count exceeded {max_properties}"
                )));
            }
            let named_property = ObjectProperty::from_descriptor(update.complete_for_new());
            let enumerable_update = named_property.is_enumerable().then_some((false, true));
            self.push_named_property(property, named_property)?;
            enumerable_update
        };
        if let Some((was_enumerable, is_enumerable)) = enumerable_update {
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
        }
        Ok(())
    }

    fn define_array_property(
        &mut self,
        index: ArrayIndex,
        property: PropertyKey,
        update: DataPropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        let Some(position) = index.dense_position(max_properties)? else {
            self.sparse_array_keys.insert(index, property);
            return self.define_named_property(property, update, max_properties);
        };
        if self.array_elements.get(position).is_none() {
            let new_len = position
                .checked_add(1)
                .ok_or_else(|| crate::error::Error::limit(super::ARRAY_INDEX_LIMIT_ERROR))?;
            self.array_elements.resize_with(new_len, || None);
        }

        let has_existing = self
            .array_elements
            .get(position)
            .and_then(Option::as_ref)
            .is_some();
        if !has_existing && self.property_count() >= max_properties {
            return Err(crate::error::Error::limit(format!(
                "object property count exceeded {max_properties}"
            )));
        }
        let slot = self
            .array_elements
            .get_mut(position)
            .ok_or_else(|| crate::error::Error::runtime("array index storage is not available"))?;
        if let Some(existing) = slot {
            let was_enumerable = existing.is_enumerable();
            existing.define(update);
            let is_enumerable = existing.is_enumerable();
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
        } else {
            let property = ObjectProperty::from_descriptor(update.complete_for_new());
            if property.is_enumerable() {
                self.enumerable_property_count = self.enumerable_property_count.saturating_add(1);
            }
            *slot = Some(property);
            self.array_property_count = self.array_property_count.saturating_add(1);
        }
        self.extend_array_length(index)
    }

    fn array_element_descriptor(&self, index: ArrayIndex) -> Option<DataPropertyDescriptor> {
        let position = index.position().ok()?;
        self.array_elements
            .get(position)
            .and_then(Option::as_ref)
            .map(ObjectProperty::descriptor)
    }
}
