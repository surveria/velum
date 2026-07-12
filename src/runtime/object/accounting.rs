use crate::error::{Error, Result};

use super::ObjectHeap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ObjectStorageCounts {
    objects: usize,
    properties: usize,
    byte_buffers: usize,
    cache_entries: usize,
    associations: usize,
    object_payload_bytes: usize,
    byte_buffer_payload_bytes: usize,
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
            cache_entries: self.shapes.storage_entry_count()?,
            associations: usize::from(self.object_prototype.is_some())
                .checked_add(usize::from(self.array_prototype.is_some()))
                .and_then(|count| count.checked_add(internal_associations))
                .ok_or_else(|| Error::limit("object anchor association count overflowed"))?,
            object_payload_bytes: self.object_payload_bytes,
            byte_buffer_payload_bytes: self.byte_buffer_payload_bytes,
        })
    }
}

impl super::Object {
    pub(super) fn storage_payload_bytes(&self) -> Result<(usize, usize, usize)> {
        let object_payload_bytes = if let Some(regexp) = &self.regexp_value {
            regexp
                .pattern()
                .len()
                .checked_add(regexp.flags().len())
                .ok_or_else(|| Error::limit("object payload bytes overflowed"))?
        } else {
            0
        };
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
