use crate::{
    error::{Error, Result},
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectHeap, ObjectProperty,
        OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable, PropertyUpdate,
        PropertyWritable,
    },
    value::ObjectId,
};

use super::ArrayLength;

impl ObjectHeap {
    /// Sets an array length while preserving element deletion and rollback semantics.
    pub(crate) fn set_array_length(&mut self, id: ObjectId, new_length: usize) -> Result<()> {
        if !self.object(id)?.array_length_writable.is_yes() {
            return Err(Error::type_error("array length is not writable"));
        }
        self.apply_array_length(id, new_length)
    }

    fn apply_array_length(&mut self, id: ObjectId, new_length: usize) -> Result<()> {
        let Some(current) = self.array_len_if_array(id)? else {
            return Err(Error::runtime(
                "set_array_length requires an array receiver",
            ));
        };
        if new_length < current {
            let indices = self
                .object(id)?
                .array_storage
                .indices_at_or_above(new_length)?;
            for array_index in indices.into_iter().rev() {
                if !self.delete_array_index(id, array_index)? {
                    let restored = array_index
                        .position()?
                        .checked_add(1)
                        .ok_or_else(|| Error::limit("array length restoration overflowed"))?;
                    self.object_mut(id)?.array_length = Some(ArrayLength::from_usize(restored)?);
                    return Err(Error::type_error(
                        "array length shrink encountered a non-configurable element",
                    ));
                }
            }
        }
        self.object_mut(id)?.array_length = Some(ArrayLength::from_usize(new_length)?);
        Ok(())
    }

    pub(crate) fn define_array_length_property(
        &mut self,
        id: ObjectId,
        update: DataPropertyUpdate,
        new_length: Option<usize>,
    ) -> Result<()> {
        let object = self.object(id)?;
        let Some(current_length) = object.array_length else {
            return Err(Error::runtime(
                "array length definition requires an array receiver",
            ));
        };
        let make_nonwritable = update.writable() == Some(PropertyWritable::No);
        let mut current = ObjectProperty::from_descriptor(DataPropertyDescriptor::new(
            current_length.value(),
            object.array_length_writable,
            PropertyEnumerable::No,
            PropertyConfigurable::No,
        ));
        current.define(PropertyUpdate::Data(update))?;
        let OwnPropertyDescriptor::Data(updated) = current.own_descriptor() else {
            return Err(Error::runtime(
                "array length descriptor changed to an accessor",
            ));
        };
        if let Some(new_length) = new_length
            && let Err(error) = self.apply_array_length(id, new_length)
        {
            if make_nonwritable {
                self.object_mut(id)?.array_length_writable = PropertyWritable::No;
            }
            return Err(error);
        }
        self.object_mut(id)?.array_length_writable = updated.writable();
        Ok(())
    }
}
