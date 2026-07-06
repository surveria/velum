use crate::value::Value;

use super::shape::ShapePropertyAttributes;
use super::{
    ARRAY_LENGTH_PROPERTY, ArrayIndex, Object, ObjectHeap, PropertyKey, PropertyLookup, ShapeTable,
};
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
    version: u64,
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
            version: 0,
        }
    }

    const fn from_descriptor(descriptor: DataPropertyDescriptor) -> Self {
        Self {
            descriptor,
            version: 0,
        }
    }

    pub fn value(&self) -> Value {
        self.descriptor.value()
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub const fn is_enumerable(&self) -> bool {
        self.descriptor.enumerable().is_yes()
    }

    pub const fn is_configurable(&self) -> bool {
        self.descriptor.configurable().is_yes()
    }

    pub(super) const fn has_default_array_attributes(&self) -> bool {
        self.descriptor.writable().is_yes()
            && self.descriptor.enumerable().is_yes()
            && self.descriptor.configurable().is_yes()
    }

    pub fn descriptor(&self) -> DataPropertyDescriptor {
        self.descriptor.clone()
    }

    pub(super) const fn shape_attributes(&self) -> ShapePropertyAttributes {
        ShapePropertyAttributes::new(
            self.descriptor.writable().is_yes(),
            self.descriptor.enumerable().is_yes(),
            self.descriptor.configurable().is_yes(),
        )
    }

    pub fn set_value(&mut self, value: Value) {
        if self.descriptor.writable().is_yes() {
            self.descriptor.value = value;
            self.version = self.version.saturating_add(1);
        }
    }

    pub fn define(&mut self, update: DataPropertyUpdate) {
        if let Some(value) = update.value {
            self.descriptor.value = value;
            self.version = self.version.saturating_add(1);
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
            .and_then(|object| object.own_property_descriptor(property, &self.shapes))
    }

    pub fn define_property(
        &mut self,
        id: ObjectId,
        property: PropertyKey,
        property_name: &str,
        update: DataPropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        let before = self.object(id)?.structure_snapshot();
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        object.define_property(property, property_name, update, shapes, max_properties)?;
        self.bump_if_structure_changed(id, before)
    }

    pub fn has_own(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        self.object(id)
            .and_then(|object| object.has_own(property, &self.shapes))
    }
}

impl Object {
    fn own_property_descriptor(
        &self,
        property: PropertyLookup<'_>,
        shapes: &ShapeTable,
    ) -> Result<Option<DataPropertyDescriptor>> {
        if let Some(length) = self
            .array_length
            .filter(|_| property.name() == ARRAY_LENGTH_PROPERTY)
        {
            return Ok(Some(DataPropertyDescriptor::new(
                length.value(),
                PropertyWritable::Yes,
                PropertyEnumerable::No,
                PropertyConfigurable::No,
            )));
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property.name())
            && let Some(descriptor) = self.array_element_descriptor(index)
        {
            return Ok(Some(descriptor));
        }
        let Some(key) = property.key() else {
            return Ok(None);
        };
        self.named_property(shapes, key)
            .map(|property| property.map(ObjectProperty::descriptor))
    }

    fn define_property(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        update: DataPropertyUpdate,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        let index = ArrayIndex::parse(property_name);
        if self.has_virtual_string_property_name(property_name)? {
            return Ok(());
        }
        if self.array_length.is_some()
            && let Some(index) = index
        {
            return self.define_array_property(index, property, update, shapes, max_properties);
        }
        self.define_named_property(property, update, shapes, max_properties)?;
        if let Some(index) = index {
            self.array_storage.insert_sparse_key(index, property);
        }
        Ok(())
    }

    fn define_named_property(
        &mut self,
        property: PropertyKey,
        update: DataPropertyUpdate,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        let property_count = self.property_count();
        let enumerable_update = if self.contains_named_property(shapes, property)? {
            let (was_enumerable, is_enumerable, attributes) = {
                let existing = self.named_property_mut(shapes, property)?;
                let was_enumerable = existing.is_enumerable();
                existing.define(update);
                (
                    was_enumerable,
                    existing.is_enumerable(),
                    existing.shape_attributes(),
                )
            };
            self.shape = shapes.transition_after_update(self.shape, property, attributes)?;
            Some((was_enumerable, is_enumerable))
        } else {
            if property_count >= max_properties {
                return Err(crate::error::Error::limit(format!(
                    "object property count exceeded {max_properties}"
                )));
            }
            let named_property = ObjectProperty::from_descriptor(update.complete_for_new());
            let enumerable_update = named_property.is_enumerable().then_some((false, true));
            self.push_named_property(shapes, property, named_property)?;
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
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        if index.dense_position(max_properties)?.is_none() {
            self.array_storage.insert_sparse_key(index, property);
            return self.define_named_property(property, update, shapes, max_properties);
        }

        let has_existing = self.array_storage.dense_property(index).is_some();
        if !has_existing && self.property_count() >= max_properties {
            return Err(crate::error::Error::limit(format!(
                "object property count exceeded {max_properties}"
            )));
        }
        if let Some(existing) = self.array_storage.dense_property_mut(index)? {
            let was_enumerable = existing.is_enumerable();
            existing.define(update);
            let is_enumerable = existing.is_enumerable();
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
        } else {
            let property = ObjectProperty::from_descriptor(update.complete_for_new());
            let is_enumerable = property.is_enumerable();
            let previous = self.array_storage.insert_dense_property(index, property)?;
            if previous.is_some() {
                return Err(crate::error::Error::runtime(
                    "array index storage replaced existing slot",
                ));
            }
            if is_enumerable {
                self.enumerable_property_count = self.enumerable_property_count.saturating_add(1);
            }
        }
        self.extend_array_length(index)
    }

    fn array_element_descriptor(&self, index: ArrayIndex) -> Option<DataPropertyDescriptor> {
        self.array_storage
            .dense_property(index)
            .map(ObjectProperty::descriptor)
    }
}
