use std::collections::BTreeMap;

use crate::error::{Error, Result};
use crate::value::{ObjectId, Value};

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

        let mut object = Object::new();
        for (key, value) in properties {
            object.set(key, value, max_properties)?;
        }

        let id = ObjectId::new(self.objects.len());
        self.objects.push(object);
        Ok(Value::Object(id))
    }

    pub fn get(&self, id: ObjectId, property: &str) -> Result<Value> {
        let object = self.object(id)?;
        Ok(object.get(property).cloned().unwrap_or(Value::Undefined))
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
}

impl Object {
    const fn new() -> Self {
        Self {
            properties: BTreeMap::new(),
        }
    }

    fn get(&self, property: &str) -> Option<&Value> {
        self.properties.get(property)
    }

    fn set(&mut self, property: String, value: Value, max_properties: usize) -> Result<()> {
        if !self.properties.contains_key(&property) && self.properties.len() >= max_properties {
            return Err(Error::limit(format!(
                "object property count exceeded {max_properties}"
            )));
        }
        self.properties.insert(property, value);
        Ok(())
    }
}
