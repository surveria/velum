use crate::{runtime::abstract_operations::same_value, value::Value};

use super::{
    AccessorPropertyUpdate, DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable,
    PropertyEnumerable, PropertyUpdate, PropertyWritable,
};

impl PropertyUpdate {
    pub(in crate::runtime) const fn configurable(&self) -> Option<PropertyConfigurable> {
        match self {
            Self::Data(update) => update.configurable,
            Self::Accessor(update) => update.configurable,
        }
    }

    pub(in crate::runtime) const fn writable(&self) -> Option<PropertyWritable> {
        match self {
            Self::Data(update) => update.writable,
            Self::Accessor(_) => None,
        }
    }
}

impl OwnPropertyDescriptor {
    pub(in crate::runtime) const fn configurable(&self) -> PropertyConfigurable {
        match self {
            Self::Data(descriptor) => descriptor.configurable(),
            Self::Accessor(descriptor) => descriptor.configurable(),
        }
    }

    pub(in crate::runtime) const fn writable(&self) -> Option<PropertyWritable> {
        match self {
            Self::Data(descriptor) => Some(descriptor.writable()),
            Self::Accessor(_) => None,
        }
    }

    pub(in crate::runtime) const fn data_value_ref(&self) -> Option<&Value> {
        match self {
            Self::Data(descriptor) => Some(descriptor.value_ref()),
            Self::Accessor(_) => None,
        }
    }

    pub(in crate::runtime) const fn accessor_get_ref(&self) -> Option<&Value> {
        match self {
            Self::Accessor(descriptor) => Some(descriptor.get_ref()),
            Self::Data(_) => None,
        }
    }

    pub(in crate::runtime) const fn accessor_set_ref(&self) -> Option<&Value> {
        match self {
            Self::Accessor(descriptor) => Some(descriptor.set_ref()),
            Self::Data(_) => None,
        }
    }
}

pub(in crate::runtime) fn is_compatible_property_update(
    extensible: bool,
    update: &PropertyUpdate,
    current: Option<&OwnPropertyDescriptor>,
) -> bool {
    let Some(current) = current else {
        return extensible;
    };
    if update_is_empty(update) {
        return true;
    }
    let (enumerable, configurable) = update_attributes(update);
    if !current.configurable().is_yes()
        && (configurable == Some(PropertyConfigurable::Yes)
            || enumerable.is_some_and(|value| value.is_yes() != descriptor_enumerable(current)))
    {
        return false;
    }
    match (current, update) {
        (_, PropertyUpdate::Data(data)) if data.is_generic() => true,
        (OwnPropertyDescriptor::Data(current), PropertyUpdate::Data(update)) => {
            current.configurable().is_yes()
                || current.writable().is_yes()
                || (update.writable != Some(PropertyWritable::Yes)
                    && update
                        .value
                        .as_ref()
                        .is_none_or(|value| same_value(current.value_ref(), value)))
        }
        (OwnPropertyDescriptor::Accessor(current), PropertyUpdate::Accessor(update)) => {
            current.configurable().is_yes()
                || (update
                    .get
                    .as_ref()
                    .is_none_or(|value| same_value(current.get_ref(), value))
                    && update
                        .set
                        .as_ref()
                        .is_none_or(|value| same_value(current.set_ref(), value)))
        }
        _ => current.configurable().is_yes(),
    }
}

pub(in crate::runtime) fn is_compatible_own_property_descriptor(
    extensible: bool,
    descriptor: &OwnPropertyDescriptor,
    current: Option<&OwnPropertyDescriptor>,
) -> bool {
    let update = match descriptor {
        OwnPropertyDescriptor::Data(descriptor) => PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(descriptor.value()),
            Some(descriptor.writable()),
            Some(descriptor.enumerable()),
            Some(descriptor.configurable()),
        )),
        OwnPropertyDescriptor::Accessor(descriptor) => {
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(descriptor.get()),
                Some(descriptor.set()),
                Some(descriptor.enumerable()),
                Some(descriptor.configurable()),
            ))
        }
    };
    is_compatible_property_update(extensible, &update, current)
}

const fn update_attributes(
    update: &PropertyUpdate,
) -> (Option<PropertyEnumerable>, Option<PropertyConfigurable>) {
    match update {
        PropertyUpdate::Data(update) => (update.enumerable, update.configurable),
        PropertyUpdate::Accessor(update) => (update.enumerable, update.configurable),
    }
}

const fn update_is_empty(update: &PropertyUpdate) -> bool {
    match update {
        PropertyUpdate::Data(update) => {
            update.value.is_none()
                && update.writable.is_none()
                && update.enumerable.is_none()
                && update.configurable.is_none()
        }
        PropertyUpdate::Accessor(update) => {
            update.get.is_none()
                && update.set.is_none()
                && update.enumerable.is_none()
                && update.configurable.is_none()
        }
    }
}

const fn descriptor_enumerable(descriptor: &OwnPropertyDescriptor) -> bool {
    match descriptor {
        OwnPropertyDescriptor::Data(descriptor) => descriptor.enumerable().is_yes(),
        OwnPropertyDescriptor::Accessor(descriptor) => descriptor.enumerable().is_yes(),
    }
}
