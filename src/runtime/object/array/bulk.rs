use super::{Object, ObjectProperty};
use crate::error::{Error, Result};
use crate::value::Value;

impl Object {
    pub(in crate::runtime::object) fn append_packed_default_value_iter(
        &mut self,
        values: impl IntoIterator<Item = Value>,
        value_count: usize,
        max_properties: usize,
    ) -> Result<()> {
        let reservation = self.reserve_property_growth_by(value_count)?;
        let count = self.array_storage.append_packed_default_value_iter(
            values,
            value_count,
            max_properties,
        )?;
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        self.add_enumerable_properties(count)
    }

    pub(in crate::runtime::object) fn pop_packed_for_len_if_configurable(
        &mut self,
        len: usize,
    ) -> Result<Option<ObjectProperty>> {
        let Some(property) = self.array_storage.pop_packed_for_len_if_configurable(len) else {
            return Ok(None);
        };
        if property.is_enumerable() {
            self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
        }
        self.release_property()?;
        Ok(Some(property))
    }

    fn add_enumerable_properties(&mut self, count: usize) -> Result<()> {
        self.enumerable_property_count = self
            .enumerable_property_count
            .checked_add(count)
            .ok_or_else(|| Error::limit("object enumerable property count overflowed"))?;
        Ok(())
    }
}
