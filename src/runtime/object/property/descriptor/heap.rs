use crate::{error::Result, runtime::object::ObjectHeap, value::ObjectId};

use super::{
    DataPropertyDescriptor, OwnPropertyDescriptor, PropertyKey, PropertyLookup, PropertyUpdate,
    PropertyWritable,
};

impl ObjectHeap {
    pub fn own_property_descriptor(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let descriptor = self
            .object(id)
            .and_then(|object| object.own_property_descriptor(property, &self.shapes))?;
        let Some(cell) = self.argument_parameter_cell(id, property.name())? else {
            return Ok(descriptor);
        };
        let Some(OwnPropertyDescriptor::Data(descriptor)) = descriptor else {
            return Ok(descriptor);
        };
        Ok(Some(OwnPropertyDescriptor::Data(
            DataPropertyDescriptor::new(
                cell.value(property.name())?,
                descriptor.writable(),
                descriptor.enumerable(),
                descriptor.configurable(),
            ),
        )))
    }

    pub fn define_property(
        &mut self,
        id: ObjectId,
        property: PropertyKey,
        property_name: &str,
        mut update: PropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        let before = self.object(id)?.structure_snapshot();
        let mapped = self.argument_parameter_cell(id, property_name)?;
        if let (Some(cell), PropertyUpdate::Data(data)) = (&mapped, &mut update)
            && data.writable() == Some(PropertyWritable::No)
            && data.value().is_none()
        {
            data.replace_value(cell.value(property_name)?);
        }
        let mapped_value = match &update {
            PropertyUpdate::Data(data) => data.value(),
            PropertyUpdate::Accessor(_) => None,
        };
        let remove_mapping = matches!(&update, PropertyUpdate::Accessor(_))
            || matches!(
                &update,
                PropertyUpdate::Data(data)
                    if data.writable() == Some(PropertyWritable::No)
            );
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        object.define_property(property, property_name, update, shapes, max_properties)?;
        if let (Some(cell), Some(value)) = (mapped, mapped_value) {
            cell.assign(property_name, value)?;
        }
        if remove_mapping {
            self.remove_argument_parameter_mapping(id, property_name)?;
        }
        self.bump_prototype_lookup_version()?;
        self.bump_if_structure_changed(id, &before)
    }

    pub fn has_own(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        self.object(id)
            .and_then(|object| object.has_own(property, &self.shapes))
    }
}
