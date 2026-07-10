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
        let mut byte_buffers = 0_usize;
        let mut object_payload_bytes = 0_usize;
        let mut byte_buffer_payload_bytes = 0_usize;
        for object in &self.objects {
            properties = properties
                .checked_add(object.named_properties.len())
                .and_then(|count| count.checked_add(object.array_storage.property_count()))
                .ok_or_else(|| Error::limit("object property count overflowed"))?;
            byte_buffers = byte_buffers
                .checked_add(usize::from(object.byte_buffer.is_some()))
                .ok_or_else(|| Error::limit("byte buffer count overflowed"))?;
            if let Some(regexp) = &object.regexp_value {
                object_payload_bytes = object_payload_bytes
                    .checked_add(regexp.pattern().len())
                    .and_then(|bytes| bytes.checked_add(regexp.flags().len()))
                    .ok_or_else(|| Error::limit("object payload bytes overflowed"))?;
            }
            if let Some(buffer) = &object.byte_buffer {
                byte_buffer_payload_bytes = byte_buffer_payload_bytes
                    .checked_add(buffer.byte_length())
                    .ok_or_else(|| Error::limit("byte buffer payload bytes overflowed"))?;
            }
        }
        Ok(ObjectStorageCounts {
            objects: self.objects.len(),
            properties,
            byte_buffers,
            cache_entries: self.shapes.storage_entry_count()?,
            associations: usize::from(self.object_prototype.is_some())
                .checked_add(usize::from(self.array_prototype.is_some()))
                .ok_or_else(|| Error::limit("object anchor association count overflowed"))?,
            object_payload_bytes,
            byte_buffer_payload_bytes,
        })
    }
}
