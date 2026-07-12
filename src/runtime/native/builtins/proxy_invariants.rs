use crate::{
    error::{Error, Result},
    runtime::{
        abstract_operations::same_value,
        object::{
            OwnPropertyDescriptor, PropertyConfigurable, PropertyUpdate, PropertyWritable,
            is_compatible_own_property_descriptor, is_compatible_property_update,
        },
    },
    value::Value,
};

const PROXY_INVARIANT_ERROR: &str = "proxy trap violated a target invariant";

pub(super) fn validate_get(
    descriptor: Option<&OwnPropertyDescriptor>,
    result: &Value,
) -> Result<()> {
    let Some(descriptor) = descriptor else {
        return Ok(());
    };
    if descriptor.configurable().is_yes() {
        return Ok(());
    }
    if let Some(value) = descriptor.data_value_ref()
        && descriptor.writable() == Some(PropertyWritable::No)
        && !same_value(value, result)
    {
        return invariant_error();
    }
    if descriptor
        .accessor_get_ref()
        .is_some_and(|getter| matches!(getter, Value::Undefined))
        && !matches!(result, Value::Undefined)
    {
        return invariant_error();
    }
    Ok(())
}

pub(super) fn validate_set(
    descriptor: Option<&OwnPropertyDescriptor>,
    value: &Value,
) -> Result<()> {
    let Some(descriptor) = descriptor else {
        return Ok(());
    };
    if descriptor.configurable().is_yes() {
        return Ok(());
    }
    if let Some(current) = descriptor.data_value_ref()
        && descriptor.writable() == Some(PropertyWritable::No)
        && !same_value(current, value)
    {
        return invariant_error();
    }
    if descriptor
        .accessor_set_ref()
        .is_some_and(|setter| matches!(setter, Value::Undefined))
    {
        return invariant_error();
    }
    Ok(())
}

pub(super) fn validate_has(
    descriptor: Option<&OwnPropertyDescriptor>,
    extensible: bool,
) -> Result<()> {
    if descriptor.is_some_and(|descriptor| !descriptor.configurable().is_yes())
        || (descriptor.is_some() && !extensible)
    {
        return invariant_error();
    }
    Ok(())
}

pub(super) fn validate_delete(
    descriptor: Option<&OwnPropertyDescriptor>,
    extensible: bool,
) -> Result<()> {
    validate_has(descriptor, extensible)
}

pub(super) fn validate_get_prototype(
    extensible: bool,
    target_prototype: &Value,
    result: &Value,
) -> Result<()> {
    if !extensible && !same_value(target_prototype, result) {
        return invariant_error();
    }
    Ok(())
}

pub(super) fn validate_set_prototype(
    extensible: bool,
    target_prototype: &Value,
    requested: &Value,
) -> Result<()> {
    if !extensible && !same_value(target_prototype, requested) {
        return invariant_error();
    }
    Ok(())
}

pub(super) fn validate_is_extensible(target: bool, result: bool) -> Result<()> {
    if target != result {
        return invariant_error();
    }
    Ok(())
}

pub(super) fn validate_prevent_extensions(target_is_extensible: bool) -> Result<()> {
    if target_is_extensible {
        return invariant_error();
    }
    Ok(())
}

pub(super) fn validate_define_property(
    update: &PropertyUpdate,
    target: Option<&OwnPropertyDescriptor>,
    extensible: bool,
) -> Result<()> {
    if !is_compatible_property_update(extensible, update, target) {
        return invariant_error();
    }
    if target.is_none() && update.configurable() == Some(PropertyConfigurable::No) {
        return invariant_error();
    }
    let Some(target) = target else {
        return Ok(());
    };
    if update.configurable() == Some(PropertyConfigurable::No) && target.configurable().is_yes() {
        return invariant_error();
    }
    if !target.configurable().is_yes()
        && target.writable() == Some(PropertyWritable::Yes)
        && update.writable() == Some(PropertyWritable::No)
    {
        return invariant_error();
    }
    Ok(())
}

pub(super) fn validate_get_own_property_descriptor(
    result: Option<&OwnPropertyDescriptor>,
    target: Option<&OwnPropertyDescriptor>,
    extensible: bool,
) -> Result<()> {
    let Some(result) = result else {
        return validate_has(target, extensible);
    };
    if !is_compatible_own_property_descriptor(extensible, result, target) {
        return invariant_error();
    }
    if result.configurable().is_yes() {
        return Ok(());
    }
    let Some(target) = target else {
        return invariant_error();
    };
    if target.configurable().is_yes()
        || (result.writable() == Some(PropertyWritable::No)
            && target.writable() == Some(PropertyWritable::Yes))
    {
        return invariant_error();
    }
    Ok(())
}

fn invariant_error<T>() -> Result<T> {
    Err(Error::type_error(PROXY_INVARIANT_ERROR))
}
