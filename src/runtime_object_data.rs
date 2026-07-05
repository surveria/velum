use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

impl ObjectHeap {
    pub(crate) fn create_data_object(
        &mut self,
        properties: Vec<(String, Value)>,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary_with_property_capacity(properties.len());
        object.prototype = Some(self.object_prototype_id(max_objects, max_properties)?);
        for (key, value) in properties {
            object.set_ordinary(key, value, max_properties)?;
        }
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(crate) fn create_empty_data_object(
        &mut self,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<ObjectId> {
        let mut object = Object::ordinary();
        object.prototype = Some(self.object_prototype_id(max_objects, max_properties)?);
        self.push_object(object, max_objects)
    }
}
