use crate::{error::Result, value::ObjectId};

use super::{Object, ObjectExtensibility, ObjectHeap, PropertyWritable, ShapeTable};

impl ObjectHeap {
    pub(crate) fn prevent_extensions(&mut self, id: ObjectId) -> Result<()> {
        let before = self.object(id)?.structure_snapshot();
        self.object_mut(id)?.prevent_extensions();
        self.bump_if_structure_changed(id, before)
    }

    pub(crate) fn seal(&mut self, id: ObjectId) -> Result<()> {
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        object.seal(shapes)?;
        self.bump_prototype_lookup_version()
    }

    pub(crate) fn freeze(&mut self, id: ObjectId) -> Result<()> {
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        object.freeze(shapes)?;
        self.bump_prototype_lookup_version()
    }

    pub(crate) fn is_extensible(&self, id: ObjectId) -> Result<bool> {
        Ok(self.object(id)?.is_extensible())
    }

    pub(crate) fn is_sealed(&self, id: ObjectId) -> Result<bool> {
        Ok(self.object(id)?.is_sealed())
    }

    pub(crate) fn is_frozen(&self, id: ObjectId) -> Result<bool> {
        Ok(self.object(id)?.is_frozen())
    }
}

impl Object {
    const fn is_extensible(&self) -> bool {
        self.extensibility.is_extensible()
    }

    const fn prevent_extensions(&mut self) {
        self.extensibility = ObjectExtensibility::NonExtensible;
    }

    fn seal(&mut self, shapes: &mut ShapeTable) -> Result<()> {
        self.prevent_extensions();
        self.seal_named_properties(shapes)?;
        self.array_storage.seal_dense_properties();
        Ok(())
    }

    fn freeze(&mut self, shapes: &mut ShapeTable) -> Result<()> {
        self.prevent_extensions();
        self.freeze_named_properties(shapes)?;
        self.array_storage.freeze_dense_properties();
        if self.array_length.is_some() {
            self.array_length_writable = PropertyWritable::No;
        }
        Ok(())
    }

    fn is_sealed(&self) -> bool {
        !self.is_extensible()
            && self
                .named_properties
                .iter()
                .all(|entry| !entry.property().is_configurable())
            && self.array_storage.dense_properties_are_sealed()
    }

    fn is_frozen(&self) -> bool {
        self.is_sealed()
            && self
                .named_properties
                .iter()
                .all(|entry| entry.property().is_frozen())
            && self.array_storage.dense_properties_are_frozen()
            && self
                .array_length
                .is_none_or(|_| !self.array_length_writable.is_yes())
    }

    fn seal_named_properties(&mut self, shapes: &mut ShapeTable) -> Result<()> {
        for entry in &mut self.named_properties {
            let key = entry.key();
            entry.property_mut().seal();
            let attributes = entry.property().shape_attributes();
            self.shape = shapes.transition_after_update(self.shape, key, attributes)?;
        }
        Ok(())
    }

    fn freeze_named_properties(&mut self, shapes: &mut ShapeTable) -> Result<()> {
        for entry in &mut self.named_properties {
            let key = entry.key();
            entry.property_mut().freeze();
            let attributes = entry.property().shape_attributes();
            self.shape = shapes.transition_after_update(self.shape, key, attributes)?;
        }
        Ok(())
    }
}
