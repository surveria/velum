use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DateValue {
    Invalid,
    Milliseconds(i64),
}

impl DateValue {
    pub(crate) const fn from_millis(value: i64) -> Self {
        Self::Milliseconds(value)
    }

    pub(crate) const fn millis(self) -> Option<i64> {
        match self {
            Self::Invalid => None,
            Self::Milliseconds(value) => Some(value),
        }
    }
}

impl ObjectHeap {
    pub(crate) fn create_date_object(
        &mut self,
        value: DateValue,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<Value> {
        let mut object = Object::date(value);
        object.prototype = Some(prototype);
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(crate) fn date_value(&self, id: ObjectId) -> Result<Option<DateValue>> {
        Ok(self.object(id)?.date_value)
    }

    pub(crate) fn set_date_value(&mut self, id: ObjectId, value: DateValue) -> Result<()> {
        self.object_mut(id)?.date_value = Some(value);
        Ok(())
    }
}

impl Object {
    pub(super) const fn date(value: DateValue) -> Self {
        Self {
            named_properties: Vec::new(),
            array_storage: super::ArrayStorage::new(),
            shape: super::ShapeId::root(),
            enumerable_property_count: 0,
            array_length: None,
            array_length_writable: super::PropertyWritable::Yes,
            string_value: None,
            primitive_value: None,
            date_value: Some(value),
            regexp_value: None,
            proxy_value: None,
            is_raw_json: false,
            prototype: None,
            extensibility: super::ObjectExtensibility::Extensible,
        }
    }
}
