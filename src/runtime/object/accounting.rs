use crate::error::{Error, Result};

use super::ObjectHeap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ObjectStorageCounts {
    objects: usize,
    properties: usize,
    byte_buffers: usize,
    host_instances: usize,
    host_payloads: usize,
    cache_entries: usize,
    associations: usize,
    object_payload_bytes: usize,
    byte_buffer_payload_bytes: usize,
    host_payload_bytes: usize,
}

impl ObjectStorageCounts {
    pub(in crate::runtime) const fn objects(self) -> usize {
        self.objects
    }

    pub(in crate::runtime) const fn properties(self) -> usize {
        self.properties
    }

    pub(in crate::runtime) const fn byte_buffers(self) -> usize {
        self.byte_buffers
    }

    pub(in crate::runtime) const fn host_instances(self) -> usize {
        self.host_instances
    }

    pub(in crate::runtime) const fn host_payloads(self) -> usize {
        self.host_payloads
    }

    pub(in crate::runtime) const fn cache_entries(self) -> usize {
        self.cache_entries
    }

    pub(in crate::runtime) const fn associations(self) -> usize {
        self.associations
    }

    pub(in crate::runtime) const fn object_payload_bytes(self) -> usize {
        self.object_payload_bytes
    }

    pub(in crate::runtime) const fn byte_buffer_payload_bytes(self) -> usize {
        self.byte_buffer_payload_bytes
    }

    pub(in crate::runtime) const fn host_payload_bytes(self) -> usize {
        self.host_payload_bytes
    }
}

impl ObjectHeap {
    pub(in crate::runtime) fn storage_counts(&self) -> Result<ObjectStorageCounts> {
        let mut properties = 0_usize;
        let mut internal_associations = 0_usize;
        for (index, object) in self.objects.indexed() {
            properties = properties
                .checked_add(object.named_properties.len())
                .and_then(|count| count.checked_add(object.array_storage.property_count()))
                .and_then(|count| {
                    count.checked_add(self.private_slots.get(index).map_or(0, std::vec::Vec::len))
                })
                .ok_or_else(|| Error::limit("object property count overflowed"))?;
            internal_associations = internal_associations
                .checked_add(usize::from(object.shadow_realm.is_some()))
                .ok_or_else(|| Error::limit("object association count overflowed"))?;
        }
        Ok(ObjectStorageCounts {
            objects: self.objects.len(),
            properties,
            byte_buffers: self.byte_buffer_count,
            host_instances: self.host_payloads.instance_count(),
            host_payloads: self.host_payloads.payload_count(),
            cache_entries: self.shapes.storage_entry_count()?,
            associations: usize::from(self.object_prototype.is_some())
                .checked_add(usize::from(self.array_prototype.is_some()))
                .and_then(|count| count.checked_add(internal_associations))
                .ok_or_else(|| Error::limit("object anchor association count overflowed"))?,
            object_payload_bytes: self.object_payload_bytes,
            byte_buffer_payload_bytes: self.byte_buffer_payload_bytes,
            host_payload_bytes: self.host_payloads.logical_payload_bytes()?,
        })
    }
}

impl super::Object {
    pub(super) fn storage_payload_bytes(&self) -> Result<(usize, usize, usize)> {
        let regexp_payload_bytes = self
            .regexp_value
            .as_ref()
            .map_or(0, super::RegExpValue::storage_payload_bytes);
        let temporal_payload_bytes = self
            .temporal_value
            .as_ref()
            .map_or(0, super::TemporalValue::storage_payload_bytes);
        let intl_payload_bytes = self
            .intl_value
            .as_ref()
            .map_or(0, super::IntlValue::storage_payload_bytes);
        let object_payload_bytes = regexp_payload_bytes
            .checked_add(intl_payload_bytes)
            .and_then(|bytes| bytes.checked_add(temporal_payload_bytes))
            .ok_or_else(|| Error::limit("object payload bytes overflowed"))?;
        let byte_buffer_count = usize::from(self.byte_buffer.is_some());
        let byte_buffer_payload_bytes = self
            .byte_buffer
            .as_ref()
            .map_or(0, super::ByteBuffer::byte_length);
        Ok((
            object_payload_bytes,
            byte_buffer_count,
            byte_buffer_payload_bytes,
        ))
    }
}
