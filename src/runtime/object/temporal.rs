use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

#[derive(Debug, Clone)]
pub enum TemporalValue {
    Duration(temporal_rs::Duration),
}

impl TemporalValue {
    pub(super) const fn storage_payload_bytes() -> usize {
        std::mem::size_of::<Self>()
    }
}

impl ObjectHeap {
    pub(crate) fn create_temporal_object(
        &mut self,
        value: TemporalValue,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<Value> {
        let mut object = Object::ordinary();
        object.prototype = Some(prototype);
        object.temporal_value = Some(value);
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(crate) fn temporal_value(&self, id: ObjectId) -> Result<Option<&TemporalValue>> {
        Ok(self.object(id)?.temporal_value.as_ref())
    }
}
