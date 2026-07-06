use crate::error::{Error, Result};
use crate::value::{ObjectId, Value};

#[path = "runtime_object_array.rs"]
mod runtime_object_array;
#[path = "runtime_object_array_storage.rs"]
mod runtime_object_array_storage;
#[path = "runtime_object_base.rs"]
mod runtime_object_base;
#[path = "runtime_object_data.rs"]
mod runtime_object_data;
#[path = "runtime_object_descriptor.rs"]
mod runtime_object_descriptor;
#[path = "runtime_object_index.rs"]
mod runtime_object_index;
#[path = "runtime_object_key.rs"]
mod runtime_object_key;
#[path = "runtime_object_keys.rs"]
mod runtime_object_keys;
#[path = "runtime_object_slot.rs"]
mod runtime_object_slot;
#[path = "runtime_object_string.rs"]
mod runtime_object_string;

use runtime_object_array_storage::ArrayStorage;
use runtime_object_base::LiteralPrototype;
pub use runtime_object_base::ObjectHeap;
pub use runtime_object_descriptor::{
    DataPropertyDescriptor, DataPropertyUpdate, ObjectProperty, PropertyConfigurable,
    PropertyEnumerable, PropertyWritable,
};
use runtime_object_index::{ArrayIndex, ArrayLength};
pub use runtime_object_key::{ObjectPropertyInit, PropertyKey, PropertyLookup};

const ARRAY_LENGTH_PROPERTY: &str = "length";
const ARRAY_INDEX_LIMIT_ERROR: &str = "array index exceeded supported range";
pub const OBJECT_CONSTRUCTOR_PROPERTY: &str = "constructor";
const PROTOTYPE_PROPERTY: &str = "__proto__";

impl ObjectHeap {
    pub fn create(
        &mut self,
        properties: Vec<(PropertyKey, String, Value)>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary_with_property_capacity(properties.len());
        let mut literal_prototype = None;
        for (key, name, value) in properties {
            if name == PROTOTYPE_PROPERTY {
                if let Some(prototype) = Object::literal_prototype(&value) {
                    literal_prototype = Some(prototype);
                }
            } else {
                object.set(key, &name, value, max_properties)?;
            }
        }
        object.prototype = match literal_prototype {
            Some(prototype) => prototype.into_object_id(),
            None => Some(self.object_prototype_id(constructor_key, max_objects, max_properties)?),
        };

        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(Value::Object(id))
    }

    pub fn create_array(
        &mut self,
        elements: Vec<Value>,
        prototype: ObjectId,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let length = ArrayLength::from_usize(elements.len())?;
        let mut object = Object::array(length);
        object.prototype = Some(prototype);
        for (index, value) in elements.into_iter().enumerate() {
            let index = ArrayIndex::from_usize(index)?;
            object.set_array_property_value(index, None, value, None, max_properties)?;
        }

        self.push_object(object, max_objects).map(Value::Object)
    }

    pub fn create_array_with_length(
        &mut self,
        length: usize,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<Value> {
        let length = ArrayLength::from_usize(length)?;
        let mut object = Object::array(length);
        object.prototype = Some(prototype);
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub fn create_with_prototype(
        &mut self,
        prototype: Option<ObjectId>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        self.create_with_prototype_id(prototype, constructor_key, max_objects, max_properties)
            .map(Value::Object)
    }

    pub(crate) fn create_with_prototype_id(
        &mut self,
        prototype: Option<ObjectId>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = self.resolve_default_prototype(
            prototype,
            constructor_key,
            max_objects,
            max_properties,
        )?;
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        object.prototype = prototype;

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(id)
    }

    pub(crate) fn create_with_prototype_property(
        &mut self,
        prototype: Option<ObjectId>,
        property: ObjectPropertyInit<'_>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = self.resolve_default_prototype(
            prototype,
            constructor_key,
            max_objects,
            max_properties,
        )?;
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        object.prototype = prototype;
        object.define(
            property.key,
            property.name,
            property.value,
            property.enumerable,
            max_properties,
        )?;

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(id)
    }

    fn resolve_default_prototype(
        &mut self,
        prototype: Option<ObjectId>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Option<ObjectId>> {
        if prototype.is_some() {
            return Ok(prototype);
        }
        self.object_prototype_id(constructor_key, max_objects, max_properties)
            .map(Some)
    }

    pub(crate) fn object_prototype_id(
        &mut self,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        if let Some(id) = self.object_prototype {
            return Ok(id);
        }
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        object.define(
            constructor_key,
            OBJECT_CONSTRUCTOR_PROPERTY,
            Value::String("Object".to_owned()),
            PropertyEnumerable::No,
            max_properties,
        )?;

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        self.object_prototype = Some(id);
        Ok(id)
    }

    pub(crate) fn array_prototype_id_with_constructor(
        &mut self,
        constructor: Value,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = if let Some(id) = self.array_prototype {
            id
        } else {
            let object_prototype =
                self.object_prototype_id(constructor_key, max_objects, max_properties)?;
            if self.objects.len() >= max_objects {
                return Err(Error::limit(format!("object count exceeded {max_objects}")));
            }

            let mut object = Object::ordinary();
            object.prototype = Some(object_prototype);
            let id = ObjectId::new(self.objects.len());
            self.objects.push(object);
            self.array_prototype = Some(id);
            id
        };

        self.define_non_enumerable(
            prototype,
            constructor_key,
            OBJECT_CONSTRUCTOR_PROPERTY,
            constructor,
            max_properties,
        )?;
        Ok(prototype)
    }

    pub(crate) fn existing_array_prototype_id(&self) -> Result<ObjectId> {
        self.array_prototype
            .ok_or_else(|| Error::runtime("Array prototype is not initialized"))
    }

    pub(crate) fn define_non_enumerable(
        &mut self,
        id: ObjectId,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        let object = self.object_mut(id)?;
        object.define(
            property,
            property_name,
            value,
            PropertyEnumerable::No,
            max_properties,
        )
    }

    pub fn get(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<Value> {
        self.get_in_chain(id, property)
    }

    pub fn has(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        self.has_in_chain(id, property)
    }

    pub fn set(
        &mut self,
        id: ObjectId,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        let lookup = PropertyLookup::from_key(property_name, property);
        if property_name == PROTOTYPE_PROPERTY && !self.object(id)?.has_own(lookup) {
            return self.set_prototype(id, &value);
        }
        let object = self.object_mut(id)?;
        object.set(property, property_name, value, max_properties)
    }

    pub fn delete(&mut self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        if property.name() == PROTOTYPE_PROPERTY {
            if self.object(id)?.has_own(property) {
                let object = self.object_mut(id)?;
                return Ok(object.delete(property));
            }
            return Ok(true);
        }
        let object = self.object_mut(id)?;
        Ok(object.delete(property))
    }

    fn get_in_chain(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<Value> {
        if let Some(value) = self.property_value_in_chain(id, property)? {
            return Ok(value);
        }
        if property.name() == PROTOTYPE_PROPERTY {
            return self.prototype_value(id);
        }
        Ok(Value::Undefined)
    }

    fn property_value_in_chain(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<Value>> {
        let object = self.object(id)?;
        if let Some(value) = object.get_own(property) {
            return Ok(Some(value));
        }
        let mut current = object.prototype;
        let mut visited = Vec::new();
        visited.push(id);
        while let Some(current_id) = current {
            if visited.contains(&current_id) {
                return Err(Error::runtime("prototype cycle detected"));
            }
            visited.push(current_id);
            let object = self.object(current_id)?;
            if let Some(value) = object.get_own(property) {
                return Ok(Some(value));
            }
            current = object.prototype;
        }
        Ok(None)
    }

    fn has_in_chain(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        let object = self.object(id)?;
        if object.has_own(property) {
            return Ok(true);
        }
        let mut current = object.prototype;
        let mut visited = Vec::new();
        visited.push(id);
        while let Some(current_id) = current {
            if visited.contains(&current_id) {
                return Err(Error::runtime("prototype cycle detected"));
            }
            visited.push(current_id);
            let object = self.object(current_id)?;
            if object.has_own(property) {
                return Ok(true);
            }
            current = object.prototype;
        }
        Ok(false)
    }

    fn set_prototype(&mut self, id: ObjectId, value: &Value) -> Result<()> {
        let prototype = match value {
            Value::Object(prototype) => Some(*prototype),
            Value::Null => None,
            _ => return Ok(()),
        };
        if let Some(prototype) = prototype
            && self.prototype_chain_contains(prototype, id)?
        {
            return Err(Error::runtime("prototype cycle is not allowed"));
        }
        let object = self.object_mut(id)?;
        object.prototype = prototype;
        Ok(())
    }

    fn prototype_chain_contains(&self, start: ObjectId, target: ObjectId) -> Result<bool> {
        let mut current = Some(start);
        let mut visited = Vec::new();
        while let Some(current_id) = current {
            if current_id == target {
                return Ok(true);
            }
            if visited.contains(&current_id) {
                return Err(Error::runtime("prototype cycle detected"));
            }
            visited.push(current_id);
            current = self.object(current_id)?.prototype;
        }
        Ok(false)
    }

    fn prototype_value(&self, id: ObjectId) -> Result<Value> {
        let object = self.object(id)?;
        Ok(object.prototype.map_or(Value::Null, Value::Object))
    }

    fn object(&self, id: ObjectId) -> Result<&Object> {
        self.objects
            .get(id.index())
            .ok_or_else(|| Error::runtime("object id is not defined"))
    }

    fn object_mut(&mut self, id: ObjectId) -> Result<&mut Object> {
        self.objects
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("object id is not defined"))
    }

    fn push_object(&mut self, object: Object, max_objects: usize) -> Result<ObjectId> {
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(id)
    }
}

#[derive(Debug, Clone, Default)]
struct Object {
    properties: Vec<runtime_object_slot::PropertyIndexEntry>,
    named_properties: Vec<runtime_object_slot::NamedProperty>,
    array_storage: ArrayStorage,
    enumerable_property_count: usize,
    array_length: Option<ArrayLength>,
    prototype: Option<ObjectId>,
}

impl Object {
    const fn ordinary() -> Self {
        Self {
            properties: Vec::new(),
            named_properties: Vec::new(),
            array_storage: ArrayStorage::new(),
            enumerable_property_count: 0,
            array_length: None,
            prototype: None,
        }
    }

    fn ordinary_with_property_capacity(capacity: usize) -> Self {
        Self {
            properties: Vec::with_capacity(capacity),
            named_properties: Vec::with_capacity(capacity),
            array_storage: ArrayStorage::new(),
            enumerable_property_count: 0,
            array_length: None,
            prototype: None,
        }
    }

    const fn array(length: ArrayLength) -> Self {
        Self {
            properties: Vec::new(),
            named_properties: Vec::new(),
            array_storage: ArrayStorage::new(),
            enumerable_property_count: 0,
            array_length: Some(length),
            prototype: None,
        }
    }

    const fn literal_prototype(value: &Value) -> Option<LiteralPrototype> {
        match value {
            Value::Object(prototype) => Some(LiteralPrototype::Object(*prototype)),
            Value::Null => Some(LiteralPrototype::Null),
            Value::Undefined
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_)
            | Value::Error(_) => None,
        }
    }

    fn get_own(&self, property: PropertyLookup<'_>) -> Option<Value> {
        if let Some(length) = self
            .array_length
            .filter(|_| property.name() == ARRAY_LENGTH_PROPERTY)
        {
            return Some(length.value());
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property.name())
            && let Some(value) = self.array_element_value(index)
        {
            return Some(value);
        }
        let key = property.key()?;
        self.named_property(key).map(ObjectProperty::value)
    }

    fn has_own(&self, property: PropertyLookup<'_>) -> bool {
        (self.array_length.is_some() && property.name() == ARRAY_LENGTH_PROPERTY)
            || (self.array_length.is_some()
                && ArrayIndex::parse(property.name())
                    .is_some_and(|index| self.has_array_element(index)))
            || property
                .key()
                .is_some_and(|key| self.named_property(key).is_some())
    }

    fn set(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        if self.array_length.is_some() && property_name == ARRAY_LENGTH_PROPERTY {
            return Err(Error::runtime("array length assignment is not supported"));
        }
        let index = ArrayIndex::parse(property_name);
        self.set_ordinary(property, property_name, value, max_properties)?;
        if let Some(index) = index {
            self.extend_array_length(index)?;
        }
        Ok(())
    }

    fn set_ordinary(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        self.set_property_value(property, property_name, value, None, max_properties)
    }

    fn define(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        enumerable: PropertyEnumerable,
        max_properties: usize,
    ) -> Result<()> {
        self.set_property_value(
            property,
            property_name,
            value,
            Some(enumerable),
            max_properties,
        )
    }

    fn set_property_value(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        max_properties: usize,
    ) -> Result<()> {
        let index = ArrayIndex::parse(property_name);
        if self.array_length.is_some()
            && let Some(index) = index
        {
            self.set_array_property_value(
                index,
                Some((property, property_name)),
                value,
                enumerable,
                max_properties,
            )?;
            return self.extend_array_length(index);
        }

        self.set_named_property_value(property, value, enumerable, max_properties)?;
        if let Some(index) = index {
            self.array_storage.insert_sparse_key(index, property);
        }
        Ok(())
    }

    fn set_named_property_value(
        &mut self,
        property: PropertyKey,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        max_properties: usize,
    ) -> Result<()> {
        let property_count = self.property_count();
        let enumerable_update = if self.contains_named_property(property) {
            let existing = self.named_property_mut(property)?;
            let was_enumerable = existing.is_enumerable();
            existing.set_value(value);
            if let Some(enumerable) = enumerable {
                existing.set_enumerable(enumerable);
            }
            Some((was_enumerable, existing.is_enumerable()))
        } else {
            if property_count >= max_properties {
                return Err(Error::limit(format!(
                    "object property count exceeded {max_properties}"
                )));
            }
            let named_property =
                ObjectProperty::ordinary(value, enumerable.unwrap_or(PropertyEnumerable::Yes));
            let enumerable_update = named_property.is_enumerable().then_some((false, true));
            self.push_named_property(property, named_property)?;
            enumerable_update
        };
        if let Some((was_enumerable, is_enumerable)) = enumerable_update {
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
        }
        Ok(())
    }

    fn set_array_property_value(
        &mut self,
        index: ArrayIndex,
        property: Option<(PropertyKey, &str)>,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        max_properties: usize,
    ) -> Result<()> {
        if index.dense_position(max_properties)?.is_none() {
            let Some((property, _)) = property else {
                return Err(Error::runtime("sparse array property key is not available"));
            };
            self.array_storage.insert_sparse_key(index, property);
            return self.set_named_property_value(property, value, enumerable, max_properties);
        }

        if let Some(property) = self.array_storage.dense_property_mut(index)? {
            let was_enumerable = property.is_enumerable();
            property.set_value(value);
            if let Some(enumerable) = enumerable {
                property.set_enumerable(enumerable);
            }
            let is_enumerable = property.is_enumerable();
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
            return Ok(());
        }

        if self.property_count() >= max_properties {
            return Err(Error::limit(format!(
                "object property count exceeded {max_properties}"
            )));
        }
        let property =
            ObjectProperty::ordinary(value, enumerable.unwrap_or(PropertyEnumerable::Yes));
        let is_enumerable = property.is_enumerable();
        let previous = self.array_storage.insert_dense_property(index, property)?;
        if previous.is_some() {
            return Err(Error::runtime("array index storage replaced existing slot"));
        }
        if is_enumerable {
            self.enumerable_property_count = self.enumerable_property_count.saturating_add(1);
        }
        Ok(())
    }

    fn delete(&mut self, property: PropertyLookup<'_>) -> bool {
        if self.array_length.is_some() && property.name() == ARRAY_LENGTH_PROPERTY {
            return false;
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property.name())
            && self.delete_array_element(index)
        {
            return true;
        }
        let Some(key) = property.key() else {
            return true;
        };
        let Some(existing_property) = self.named_property(key) else {
            return true;
        };
        if !existing_property.is_configurable() {
            return false;
        }
        let Some(removed_property) = self.remove_named_property(key) else {
            return true;
        };
        if removed_property.is_enumerable() {
            self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
        }
        if let Some(index) = ArrayIndex::parse(property.name()) {
            self.array_storage.remove_sparse_key(index);
        }
        true
    }

    fn extend_array_length(&mut self, index: ArrayIndex) -> Result<()> {
        let Some(length) = self.array_length else {
            return Ok(());
        };
        if length.contains(index) {
            return Ok(());
        }
        self.array_length = Some(index.next_length()?);
        Ok(())
    }

    fn array_element_value(&self, index: ArrayIndex) -> Option<Value> {
        let position = index.position().ok()?;
        self.array_storage
            .dense_property_at_position(position)
            .map(ObjectProperty::value)
    }

    fn has_array_element(&self, index: ArrayIndex) -> bool {
        self.array_storage.dense_property(index).is_some()
    }

    fn delete_array_element(&mut self, index: ArrayIndex) -> bool {
        let Some(property) = self.array_storage.dense_property(index) else {
            return false;
        };
        if !property.is_configurable() {
            return false;
        }
        if let Ok(Some(property)) = self.array_storage.remove_dense_property(index) {
            if property.is_enumerable() {
                self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
            }
            return true;
        }
        false
    }

    const fn has_enumerable_own_keys(&self) -> bool {
        self.enumerable_property_count > 0
    }

    const fn update_enumerable_property_count(
        &mut self,
        was_enumerable: bool,
        is_enumerable: bool,
    ) {
        match (was_enumerable, is_enumerable) {
            (false, true) => {
                self.enumerable_property_count = self.enumerable_property_count.saturating_add(1);
            }
            (true, false) => {
                self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
            }
            (true, true) | (false, false) => {}
        }
    }

    const fn property_count(&self) -> usize {
        self.properties
            .len()
            .saturating_add(self.array_storage.property_count())
    }
}
