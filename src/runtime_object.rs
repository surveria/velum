use std::collections::{BTreeMap, btree_map::Entry};

use crate::error::{Error, Result};
use crate::value::{ObjectId, Value};

const ARRAY_LENGTH_PROPERTY: &str = "length";
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
}

impl ObjectHeap {
    pub const fn new() -> Self {
        Self {
            objects: Vec::new(),
            object_prototype: None,
        }
    }

    pub fn create(
        &mut self,
        properties: Vec<(String, Value)>,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary();
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
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let length = ArrayLength::from_usize(elements.len())?;
        let mut object = Object::array(length);
        object.prototype = Some(self.object_prototype_id(max_objects, max_properties)?);
        for (index, value) in elements.into_iter().enumerate() {
            let index = ArrayIndex::from_usize(index)?;
            object.set_ordinary(index.key(), value, max_properties)?;
        }

        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(Value::Object(id))
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

    fn object_prototype_id(
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

    pub fn get(&self, id: ObjectId, property: &str) -> Result<Value> {
        self.get_in_chain(id, property)
    }

    pub fn has(&self, id: ObjectId, property: &str) -> Result<bool> {
        self.has_in_chain(id, property)
    }

    pub fn keys(&self, id: ObjectId) -> Result<Vec<String>> {
        let mut keys = Vec::new();
        let mut visited = Vec::new();
        self.collect_keys(id, &mut keys, &mut visited)?;
        Ok(keys)
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
        let mut current = Some(id);
        let mut visited = Vec::new();
        while let Some(current_id) = current {
            if visited.contains(&current_id) {
                return Err(Error::runtime("prototype cycle detected"));
            }
            visited.push(current_id);
            let object = self.object(current_id)?;
            if let Some(value) = object.get_own(property) {
                return Ok(value);
            }
            current = object.prototype;
        }
        Ok(Value::Undefined)
    }

    fn has_in_chain(&self, id: ObjectId, property: &str) -> Result<bool> {
        let mut current = Some(id);
        let mut visited = Vec::new();
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

    fn collect_keys(
        &self,
        id: ObjectId,
        keys: &mut Vec<String>,
        visited: &mut Vec<ObjectId>,
    ) -> Result<()> {
        if visited.contains(&id) {
            return Err(Error::runtime("prototype cycle detected"));
        }
        visited.push(id);
        let object = self.object(id)?;
        for key in object.keys() {
            if !keys.iter().any(|existing| existing == &key) {
                keys.push(key);
            }
        }
        if let Some(prototype) = object.prototype {
            self.collect_keys(prototype, keys, visited)?;
        }
        Ok(())
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
}

#[derive(Debug, Clone, Default)]
struct Object {
    properties: BTreeMap<String, ObjectProperty>,
    property_order: Vec<String>,
    array_length: Option<ArrayLength>,
    prototype: Option<ObjectId>,
}

impl Object {
    const fn ordinary() -> Self {
        Self {
            properties: BTreeMap::new(),
            property_order: Vec::new(),
            array_length: None,
            prototype: None,
        }
    }

    const fn array(length: ArrayLength) -> Self {
        Self {
            properties: BTreeMap::new(),
            property_order: Vec::new(),
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
        self.properties.get(property).map(ObjectProperty::value)
    }

    fn has_own(&self, property: &str) -> bool {
        (self.array_length.is_some() && property == ARRAY_LENGTH_PROPERTY)
            || self.properties.contains_key(property)
    }

    fn keys(&self) -> Vec<String> {
        self.property_order
            .iter()
            .filter_map(|key| {
                self.properties
                    .get(key)
                    .filter(|property| property.is_enumerable())
                    .map(|_| key.clone())
            })
            .collect()
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
        let property_count = self.properties.len();
        match self.properties.entry(property) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().set_value(value);
                if let Some(enumerable) = enumerable {
                    entry.get_mut().set_enumerable(enumerable);
                }
            }
            Entry::Vacant(entry) => {
                if property_count >= max_properties {
                    return Err(Error::limit(format!(
                        "object property count exceeded {max_properties}"
                    )));
                }
                self.property_order.push(entry.key().clone());
                entry.insert(ObjectProperty::new(
                    value,
                    enumerable.unwrap_or(PropertyEnumerable::Yes),
                ));
            }
        }
        Ok(())
    }

    fn delete(&mut self, property: &str) -> bool {
        if self.array_length.is_some() && property == ARRAY_LENGTH_PROPERTY {
            return false;
        }
        let removed_property = self.properties.remove(property);
        if removed_property.is_some() {
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ArrayLength(u32);

impl ArrayLength {
    fn from_usize(value: usize) -> Result<Self> {
        let value = u32::try_from(value)
            .map_err(|_| Error::limit("array length exceeded supported range"))?;
        Ok(Self(value))
    }

    fn value(self) -> Value {
        Value::Number(f64::from(self.0))
    }

    const fn contains(self, index: ArrayIndex) -> bool {
        index.0 < self.0
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ArrayIndex(u32);

impl ArrayIndex {
    fn from_usize(value: usize) -> Result<Self> {
        let value = u32::try_from(value)
            .map_err(|_| Error::limit("array index exceeded supported range"))?;
        if value == u32::MAX {
            return Err(Error::limit("array index exceeded supported range"));
        }
        Ok(Self(value))
    }

    fn parse(property: &str) -> Option<Self> {
        let value = property.parse::<u32>().ok()?;
        if value == u32::MAX || value.to_string() != property {
            return None;
        }
        Some(Self(value))
    }

    fn key(self) -> String {
        self.0.to_string()
    }

    fn next_length(self) -> Result<ArrayLength> {
        self.0
            .checked_add(1)
            .map(ArrayLength)
            .ok_or_else(|| Error::limit("array length exceeded supported range"))
    }
}
