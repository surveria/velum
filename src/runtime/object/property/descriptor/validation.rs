use crate::{
    error::{Error, Result},
    runtime::abstract_operations::same_value,
};

use super::{
    ObjectProperty, ObjectPropertyPayload, PropertyConfigurable, PropertyUpdate, PropertyWritable,
};

const INCOMPATIBLE_DESCRIPTOR: &str =
    "property descriptor is incompatible with the existing non-configurable property";

impl ObjectProperty {
    pub(super) fn validate_update(&self, update: &PropertyUpdate) -> Result<()> {
        if self.is_configurable() {
            return Ok(());
        }
        let (enumerable, configurable) = match update {
            PropertyUpdate::Data(update) => (update.enumerable, update.configurable),
            PropertyUpdate::Accessor(update) => (update.enumerable, update.configurable),
        };
        if configurable == Some(PropertyConfigurable::Yes)
            || enumerable.is_some_and(|value| value.is_yes() != self.is_enumerable())
        {
            return Err(Error::type_error(INCOMPATIBLE_DESCRIPTOR));
        }
        match (&self.payload, update) {
            (ObjectPropertyPayload::Data(existing), PropertyUpdate::Data(update)) => {
                if update.is_generic() || existing.writable().is_yes() {
                    return Ok(());
                }
                if update.writable == Some(PropertyWritable::Yes)
                    || update
                        .value
                        .as_ref()
                        .is_some_and(|value| !same_value(existing.value_ref(), value))
                {
                    return Err(Error::type_error(INCOMPATIBLE_DESCRIPTOR));
                }
                Ok(())
            }
            (ObjectPropertyPayload::Accessor(existing), PropertyUpdate::Accessor(update)) => {
                if update
                    .get
                    .as_ref()
                    .is_some_and(|value| !same_value(existing.get_ref(), value))
                    || update
                        .set
                        .as_ref()
                        .is_some_and(|value| !same_value(existing.set_ref(), value))
                {
                    return Err(Error::type_error(INCOMPATIBLE_DESCRIPTOR));
                }
                Ok(())
            }
            (ObjectPropertyPayload::Accessor(_), PropertyUpdate::Data(update))
                if update.is_generic() =>
            {
                Ok(())
            }
            (ObjectPropertyPayload::Data(_), PropertyUpdate::Accessor(_))
            | (ObjectPropertyPayload::Accessor(_), PropertyUpdate::Data(_)) => {
                Err(Error::type_error(INCOMPATIBLE_DESCRIPTOR))
            }
        }
    }
}
