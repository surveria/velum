use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::value::{ObjectId, Value};

const ARRAY_LENGTH_PROPERTY: &str = "length";

#[derive(Debug, Clone, Default)]
pub struct ObjectHeap {
    objects: Vec<Object>,
}

impl ObjectHeap {
    pub const fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    pub fn create(
        &mut self,
        properties: Vec<(String, Value)>,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let mut object = Object::ordinary();
        for (key, value) in properties {
            object.set(key, value, max_properties)?;
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
        if self.objects.len() >= max_objects {
            return Err(Error::limit(format!("object count exceeded {max_objects}")));
        }

        let length = ArrayLength::from_usize(elements.len())?;
        let mut object = Object::array(length);
        for (index, value) in elements.into_iter().enumerate() {
            let index = ArrayIndex::from_usize(index)?;
            object.set_ordinary(index.key(), value, max_properties)?;
        }

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(Value::Object(id))
    }

    pub fn get(&self, id: ObjectId, property: &str) -> Result<Value> {
        let object = self.object(id)?;
        Ok(object.get(property))
    }

    pub fn set(
        &mut self,
        id: ObjectId,
        property: String,
        value: Value,
        max_properties: usize,
    ) -> Result<()> {
        let object = self.object_mut(id)?;
        object.set(property, value, max_properties)
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
    properties: BTreeMap<String, Value>,
    array_length: Option<ArrayLength>,
}

impl Object {
    const fn ordinary() -> Self {
        Self {
            properties: BTreeMap::new(),
            array_length: None,
        }
    }

    const fn array(length: ArrayLength) -> Self {
        Self {
            properties: BTreeMap::new(),
            array_length: Some(length),
        }
    }

    fn get(&self, property: &str) -> Value {
        if let Some(length) = self
            .array_length
            .filter(|_| property == ARRAY_LENGTH_PROPERTY)
        {
            return length.value();
        }
        self.properties
            .get(property)
            .cloned()
            .unwrap_or(Value::Undefined)
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
        if !self.properties.contains_key(&property) && self.properties.len() >= max_properties {
            return Err(Error::limit(format!(
                "object property count exceeded {max_properties}"
            )));
        }
        self.properties.insert(property, value);
        Ok(())
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
