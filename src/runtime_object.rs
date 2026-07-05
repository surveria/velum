use std::collections::{BTreeMap, btree_map::Entry};

use crate::error::{Error, Result};
use crate::value::{ObjectId, Value};

#[path = "runtime_object_array.rs"]
mod runtime_object_array;
#[path = "runtime_object_index.rs"]
mod runtime_object_index;
#[path = "runtime_object_keys.rs"]
mod runtime_object_keys;
#[path = "runtime_object_string.rs"]
mod runtime_object_string;

use runtime_object_index::{ArrayIndex, ArrayLength};

const ARRAY_LENGTH_PROPERTY: &str = "length";
const ARRAY_INDEX_LIMIT_ERROR: &str = "array index exceeded supported range";
const OBJECT_CONSTRUCTOR_PROPERTY: &str = "constructor";
const PROTOTYPE_PROPERTY: &str = "__proto__";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PropertyEnumerable {
    Yes,
    No,
}

impl PropertyEnumerable {
    const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum LiteralPrototype {
    Object(ObjectId),
    Null,
}

impl LiteralPrototype {
    const fn into_object_id(self) -> Option<ObjectId> {
        match self {
            Self::Object(id) => Some(id),
            Self::Null => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ObjectHeap {
    objects: Vec<Object>,
    object_prototype: Option<ObjectId>,
    array_prototype: Option<ObjectId>,
}

impl ObjectHeap {
    pub const fn new() -> Self {
        Self {
            objects: Vec::new(),
            object_prototype: None,
            array_prototype: None,
        }
    }

    pub fn create(
        &mut self,
        properties: Vec<(String, Value)>,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary_with_property_capacity(properties.len());
        let mut literal_prototype = None;
        for (key, value) in properties {
            if key == PROTOTYPE_PROPERTY {
                if let Some(prototype) = Object::literal_prototype(&value) {
                    literal_prototype = Some(prototype);
                }
            } else {
                object.set(key, value, max_properties)?;
            }
        }
        object.prototype = match literal_prototype {
            Some(prototype) => prototype.into_object_id(),
            None => Some(self.object_prototype_id(max_objects, max_properties)?),
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
            object.set_ordinary(index.key(), value, max_properties)?;
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
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        self.create_with_prototype_id(prototype, max_objects, max_properties)
            .map(Value::Object)
    }

    pub(crate) fn create_with_prototype_id(
        &mut self,
        prototype: Option<ObjectId>,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = self.resolve_default_prototype(prototype, max_objects, max_properties)?;
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
        property: String,
        value: Value,
        enumerable: PropertyEnumerable,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = self.resolve_default_prototype(prototype, max_objects, max_properties)?;
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        object.prototype = prototype;
        object.define(property, value, enumerable, max_properties)?;

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(id)
    }

    fn resolve_default_prototype(
        &mut self,
        prototype: Option<ObjectId>,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Option<ObjectId>> {
        if prototype.is_some() {
            return Ok(prototype);
        }
        self.object_prototype_id(max_objects, max_properties)
            .map(Some)
    }

    pub(crate) fn object_prototype_id(
        &mut self,
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
            OBJECT_CONSTRUCTOR_PROPERTY.to_owned(),
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
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let prototype = if let Some(id) = self.array_prototype {
            id
        } else {
            let object_prototype = self.object_prototype_id(max_objects, max_properties)?;
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
            OBJECT_CONSTRUCTOR_PROPERTY.to_owned(),
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
        property: String,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        let object = self.object_mut(id)?;
        object.define(property, value, PropertyEnumerable::No, max_properties)
    }

    pub fn get(&self, id: ObjectId, property: &str) -> Result<Value> {
        self.get_in_chain(id, property)
    }

    pub fn has(&self, id: ObjectId, property: &str) -> Result<bool> {
        self.has_in_chain(id, property)
    }

    pub fn set(
        &mut self,
        id: ObjectId,
        property: String,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        if property == PROTOTYPE_PROPERTY {
            return self.set_prototype(id, &value);
        }
        let object = self.object_mut(id)?;
        object.set(property, value, max_properties)
    }

    pub fn delete(&mut self, id: ObjectId, property: &str) -> Result<bool> {
        if property == PROTOTYPE_PROPERTY {
            self.object(id)?;
            return Ok(true);
        }
        let object = self.object_mut(id)?;
        Ok(object.delete(property))
    }

    fn get_in_chain(&self, id: ObjectId, property: &str) -> Result<Value> {
        if property == PROTOTYPE_PROPERTY {
            return self.prototype_value(id);
        }
        if let Some(value) = self.property_value_in_chain(id, property)? {
            return Ok(value);
        }
        Ok(Value::Undefined)
    }

    fn property_value_in_chain(&self, id: ObjectId, property: &str) -> Result<Option<Value>> {
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

    fn has_in_chain(&self, id: ObjectId, property: &str) -> Result<bool> {
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
    properties: BTreeMap<String, ObjectProperty>,
    property_order: Vec<String>,
    array_elements: Vec<Option<ObjectProperty>>,
    array_property_count: usize,
    enumerable_property_count: usize,
    array_length: Option<ArrayLength>,
    prototype: Option<ObjectId>,
}

impl Object {
    const fn ordinary() -> Self {
        Self {
            properties: BTreeMap::new(),
            property_order: Vec::new(),
            array_elements: Vec::new(),
            array_property_count: 0,
            enumerable_property_count: 0,
            array_length: None,
            prototype: None,
        }
    }

    fn ordinary_with_property_capacity(capacity: usize) -> Self {
        Self {
            properties: BTreeMap::new(),
            property_order: Vec::with_capacity(capacity),
            array_elements: Vec::new(),
            array_property_count: 0,
            enumerable_property_count: 0,
            array_length: None,
            prototype: None,
        }
    }

    const fn array(length: ArrayLength) -> Self {
        Self {
            properties: BTreeMap::new(),
            property_order: Vec::new(),
            array_elements: Vec::new(),
            array_property_count: 0,
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

    fn get_own(&self, property: &str) -> Option<Value> {
        if let Some(length) = self
            .array_length
            .filter(|_| property == ARRAY_LENGTH_PROPERTY)
        {
            return Some(length.value());
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property)
            && let Some(value) = self.array_element_value(index)
        {
            return Some(value);
        }
        self.properties.get(property).map(ObjectProperty::value)
    }

    fn has_own(&self, property: &str) -> bool {
        (self.array_length.is_some() && property == ARRAY_LENGTH_PROPERTY)
            || (self.array_length.is_some()
                && ArrayIndex::parse(property).is_some_and(|index| self.has_array_element(index)))
            || self.properties.contains_key(property)
    }

    fn set(&mut self, property: String, value: Value, max_properties: usize) -> Result<()> {
        if self.array_length.is_some() && property == ARRAY_LENGTH_PROPERTY {
            return Err(Error::runtime("array length assignment is not supported"));
        }
        let index = ArrayIndex::parse(&property);
        self.set_ordinary(property, value, max_properties)?;
        if let Some(index) = index {
            self.extend_array_length(index)?;
        }
        Ok(())
    }

    fn set_ordinary(
        &mut self,
        property: String,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        self.set_property_value(property, value, None, max_properties)
    }

    fn define(
        &mut self,
        property: String,
        value: Value,
        enumerable: PropertyEnumerable,
        max_properties: usize,
    ) -> Result<()> {
        self.set_property_value(property, value, Some(enumerable), max_properties)
    }

    fn set_property_value(
        &mut self,
        property: String,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        max_properties: usize,
    ) -> Result<()> {
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(&property)
        {
            self.set_array_property_value(
                index,
                Some(property),
                value,
                enumerable,
                max_properties,
            )?;
            return self.extend_array_length(index);
        }

        self.set_named_property_value(property, value, enumerable, max_properties)
    }

    fn set_named_property_value(
        &mut self,
        property: String,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        max_properties: usize,
    ) -> Result<()> {
        let property_count = self.property_count();
        let mut enumerable_update = None;
        match self.properties.entry(property) {
            Entry::Occupied(mut entry) => {
                let was_enumerable = entry.get().is_enumerable();
                entry.get_mut().set_value(value);
                if let Some(enumerable) = enumerable {
                    entry.get_mut().set_enumerable(enumerable);
                }
                enumerable_update = Some((was_enumerable, entry.get().is_enumerable()));
            }
            Entry::Vacant(entry) => {
                if property_count >= max_properties {
                    return Err(Error::limit(format!(
                        "object property count exceeded {max_properties}"
                    )));
                }
                self.property_order.push(entry.key().clone());
                let property =
                    ObjectProperty::new(value, enumerable.unwrap_or(PropertyEnumerable::Yes));
                if property.is_enumerable() {
                    enumerable_update = Some((false, true));
                }
                entry.insert(property);
            }
        }
        if let Some((was_enumerable, is_enumerable)) = enumerable_update {
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
        }
        Ok(())
    }

    fn set_array_property_value(
        &mut self,
        index: ArrayIndex,
        property: Option<String>,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        max_properties: usize,
    ) -> Result<()> {
        let Some(position) = index.dense_position(max_properties)? else {
            let property = property.unwrap_or_else(|| index.key());
            return self.set_named_property_value(property, value, enumerable, max_properties);
        };

        if self.array_elements.get(position).is_none() {
            let new_len = position
                .checked_add(1)
                .ok_or_else(|| Error::limit(ARRAY_INDEX_LIMIT_ERROR))?;
            self.array_elements.resize_with(new_len, || None);
        }

        if self.has_array_element(index) {
            let slot = self
                .array_elements
                .get_mut(position)
                .ok_or_else(|| Error::runtime("array index storage is not available"))?;
            let Some(property) = slot else {
                return Err(Error::runtime("array index storage is not initialized"));
            };
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
        let slot = self
            .array_elements
            .get_mut(position)
            .ok_or_else(|| Error::runtime("array index storage is not available"))?;
        let property = ObjectProperty::new(value, enumerable.unwrap_or(PropertyEnumerable::Yes));
        if property.is_enumerable() {
            self.enumerable_property_count = self.enumerable_property_count.saturating_add(1);
        }
        *slot = Some(property);
        self.array_property_count = self.array_property_count.saturating_add(1);
        Ok(())
    }

    fn delete(&mut self, property: &str) -> bool {
        if self.array_length.is_some() && property == ARRAY_LENGTH_PROPERTY {
            return false;
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property)
            && self.delete_array_element(index)
        {
            return true;
        }
        let removed_property = self.properties.remove(property);
        if let Some(removed_property) = removed_property {
            if removed_property.is_enumerable() {
                self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
            }
            self.property_order.retain(|key| key != property);
            return true;
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
        self.array_elements
            .get(position)
            .and_then(Option::as_ref)
            .map(ObjectProperty::value)
    }

    fn has_array_element(&self, index: ArrayIndex) -> bool {
        let Ok(position) = index.position() else {
            return false;
        };
        self.array_elements
            .get(position)
            .and_then(Option::as_ref)
            .is_some()
    }

    fn delete_array_element(&mut self, index: ArrayIndex) -> bool {
        let Ok(position) = index.position() else {
            return false;
        };
        let Some(slot) = self.array_elements.get_mut(position) else {
            return false;
        };
        if let Some(property) = slot.take() {
            if property.is_enumerable() {
                self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
            }
            self.array_property_count = self.array_property_count.saturating_sub(1);
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

    fn property_count(&self) -> usize {
        self.properties
            .len()
            .saturating_add(self.array_property_count)
    }
}

#[derive(Debug, Clone)]
struct ObjectProperty {
    value: Value,
    enumerable: PropertyEnumerable,
}

impl ObjectProperty {
    const fn new(value: Value, enumerable: PropertyEnumerable) -> Self {
        Self { value, enumerable }
    }

    fn value(&self) -> Value {
        self.value.clone()
    }

    const fn is_enumerable(&self) -> bool {
        self.enumerable.is_yes()
    }

    fn set_value(&mut self, value: Value) {
        self.value = value;
    }

    const fn set_enumerable(&mut self, enumerable: PropertyEnumerable) {
        self.enumerable = enumerable;
    }
}
