use crate::{
    error::{Error, Result},
    runtime::private::{PrivateNameId, PrivateSlot, PrivateSlotValue},
    value::ObjectId,
};

use super::ObjectHeap;

impl ObjectHeap {
    pub(in crate::runtime) fn add_private_slot(
        &mut self,
        id: ObjectId,
        name: PrivateNameId,
        value: PrivateSlotValue,
        max_properties: usize,
    ) -> Result<()> {
        let index = id.index();
        let slots = self
            .private_slots
            .get(index)
            .ok_or_else(|| Error::runtime("object private slot table is not defined"))?;
        if slots.iter().any(|slot| slot.id == name) {
            return Err(Error::type_error("private slot is already defined"));
        }
        let object = self.object(id)?;
        let projected = object
            .property_count()
            .checked_add(slots.len())
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| Error::limit("object property count overflowed"))?;
        if projected > max_properties {
            return Err(Error::limit(
                "object property count exceeded configured limit",
            ));
        }
        let reservation = object.reserve_property_growth()?;
        self.private_slots
            .get_mut(index)
            .ok_or_else(|| Error::runtime("object private slot table is not defined"))?
            .push(PrivateSlot { id: name, value });
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn private_slot(
        &self,
        id: ObjectId,
        name: &PrivateNameId,
    ) -> Result<Option<PrivateSlotValue>> {
        Ok(self
            .private_slots
            .get(id.index())
            .ok_or_else(|| Error::runtime("object private slot table is not defined"))?
            .iter()
            .find(|slot| slot.id == *name)
            .map(|slot| slot.value.clone()))
    }

    pub(in crate::runtime) fn set_private_field(
        &mut self,
        id: ObjectId,
        name: &PrivateNameId,
        value: crate::value::Value,
    ) -> Result<bool> {
        let Some(slot) = self
            .private_slots
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("object private slot table is not defined"))?
            .iter_mut()
            .find(|slot| slot.id == *name)
        else {
            return Ok(false);
        };
        let PrivateSlotValue::Field(current) = &mut slot.value else {
            return Ok(false);
        };
        *current = value;
        Ok(true)
    }

    pub(in crate::runtime) fn replace_private_slot(
        &mut self,
        id: ObjectId,
        name: &PrivateNameId,
        value: PrivateSlotValue,
    ) -> Result<()> {
        let slot = self
            .private_slots
            .get_mut(id.index())
            .ok_or_else(|| Error::runtime("object private slot table is not defined"))?
            .iter_mut()
            .find(|slot| slot.id == *name)
            .ok_or_else(|| Error::runtime("private slot disappeared"))?;
        slot.value = value;
        Ok(())
    }
}
