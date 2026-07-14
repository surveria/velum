use crate::{
    error::Result,
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

#[derive(Debug, Clone)]
pub enum TemporalValue {
    Duration(temporal_rs::Duration),
    Instant(temporal_rs::Instant),
    PlainDate(temporal_rs::PlainDate),
    PlainDateTime(temporal_rs::PlainDateTime),
    PlainMonthDay(temporal_rs::PlainMonthDay),
    PlainTime(temporal_rs::PlainTime),
    PlainYearMonth(temporal_rs::PlainYearMonth),
    ZonedDateTime(temporal_rs::ZonedDateTime),
}

impl TemporalValue {
    pub(super) fn storage_payload_bytes(&self) -> usize {
        let active_payload = match self {
            Self::Duration(value) => std::mem::size_of_val(value),
            Self::Instant(value) => std::mem::size_of_val(value),
            Self::PlainDate(value) => std::mem::size_of_val(value),
            Self::PlainDateTime(value) => std::mem::size_of_val(value),
            Self::PlainMonthDay(value) => std::mem::size_of_val(value),
            Self::PlainTime(value) => std::mem::size_of_val(value),
            Self::PlainYearMonth(value) => std::mem::size_of_val(value),
            Self::ZonedDateTime(value) => std::mem::size_of_val(value),
        };
        std::cmp::max(std::mem::size_of::<Self>(), active_payload)
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
        object.prototype = Some(Value::Object(prototype));
        object.temporal_value = Some(value);
        self.push_object(object, max_objects).map(Value::Object)
    }

    pub(crate) fn temporal_value(&self, id: ObjectId) -> Result<Option<&TemporalValue>> {
        Ok(self.object(id)?.temporal_value.as_ref())
    }
}
