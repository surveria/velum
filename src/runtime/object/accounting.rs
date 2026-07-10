use crate::error::{Error, Result};

use super::ObjectHeap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct ObjectStorageCounts {
    objects: usize,
    properties: usize,
    byte_buffers: usize,
    cache_entries: usize,
    associations: usize,
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
}

impl ObjectHeap {
    pub(in crate::runtime) fn storage_counts(&self) -> Result<ObjectStorageCounts> {
        let mut properties = 0_usize;
        let mut byte_buffers = 0_usize;
        for object in &self.objects {
            properties = properties
                .checked_add(object.named_properties.len())
                .and_then(|count| count.checked_add(object.array_storage.property_count()))
                .ok_or_else(|| Error::limit("object property count overflowed"))?;
            byte_buffers = byte_buffers
                .checked_add(usize::from(object.byte_buffer.is_some()))
                .ok_or_else(|| Error::limit("byte buffer count overflowed"))?;
        }
        Ok(ObjectStorageCounts {
            objects: self.objects.len(),
            properties,
            byte_buffers,
            cache_entries: self.shapes.storage_entry_count()?,
            associations: usize::from(self.object_prototype.is_some())
                .checked_add(usize::from(self.array_prototype.is_some()))
                .ok_or_else(|| Error::limit("object anchor association count overflowed"))?,
        })
    }
}
