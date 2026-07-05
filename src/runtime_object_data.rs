use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap, PropertyKey};

impl ObjectHeap {
    pub(crate) fn create_data_object(
        &mut self,
        properties: Vec<(PropertyKey, String, Value)>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary_with_property_capacity(properties.len());
        object.prototype =
            Some(self.object_prototype_id(constructor_key, max_objects, max_properties)?);
        for (key, name, value) in properties {
            object.set_ordinary(key, &name, value, max_properties)?;
        }
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(crate) fn create_empty_data_object(
        &mut self,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let mut object = Object::ordinary();
        object.prototype =
            Some(self.object_prototype_id(constructor_key, max_objects, max_properties)?);
        self.push_object(object, max_objects)
    }
}
