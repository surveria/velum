use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap, ObjectPropertyInit, PropertyKey};

impl ObjectHeap {
    pub(crate) fn create_data_object(
        &mut self,
        properties: Vec<ObjectPropertyInit<'_>>,
        constructor_key: PropertyKey,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary_with_property_capacity(properties.len());
        object.prototype =
            Some(self.object_prototype_id(constructor_key, max_objects, max_properties)?);
        for property in properties {
            object.define(
                property.key,
                property.name,
                property.value,
                property.enumerable,
                &mut self.shapes,
                max_properties,
            )?;
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
