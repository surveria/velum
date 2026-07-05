use crate::error::{Error, Result};
use crate::runtime_assertions::error_property;
use crate::runtime_object::ObjectHeap;
use crate::value::Value;

const NULLISH_PROPERTY_DELETE_ERROR: &str = "Cannot convert undefined or null to object";
const ERROR_NAME_PROPERTY: &str = "name";
const ERROR_MESSAGE_PROPERTY: &str = "message";

pub fn property_key(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

pub fn get_property(objects: &ObjectHeap, object: &Value, property: &str) -> Result<Value> {
    match object {
        Value::Error(error) => Ok(error_property(error, property)),
        Value::Object(id) => objects.get(*id, property),
        value => Err(Error::runtime(format!(
            "member access '{property}' is not supported for {}",
            value.type_name()
        ))),
    }
}

pub fn has_property(objects: &ObjectHeap, object: &Value, property: &str) -> Result<bool> {
    match object {
        Value::Error(_) => Ok(matches!(
            property,
            ERROR_NAME_PROPERTY | ERROR_MESSAGE_PROPERTY
        )),
        Value::Object(id) => objects.has(*id, property),
        value => Err(Error::runtime(format!(
            "operator 'in' is not supported for {}",
            value.type_name()
        ))),
    }
}

pub fn enumerable_property_keys(objects: &ObjectHeap, object: &Value) -> Result<Vec<String>> {
    match object {
        Value::Undefined | Value::Null => Err(Error::runtime(NULLISH_PROPERTY_DELETE_ERROR)),
        Value::Object(id) => objects.keys(*id),
        Value::Error(_) => Ok(vec![
            ERROR_NAME_PROPERTY.to_owned(),
            ERROR_MESSAGE_PROPERTY.to_owned(),
        ]),
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Function(_) => Ok(Vec::new()),
    }
}

pub fn set_property(
    objects: &mut ObjectHeap,
    object: &Value,
    property: String,
    value: Value,
    max_properties: usize,
) -> Result<()> {
    let Value::Object(id) = object else {
        return Err(Error::runtime(format!(
            "property assignment '{property}' is not supported for {}",
            object.type_name()
        )));
    };
    objects.set(*id, property, value, max_properties)
}

pub fn delete_property(objects: &mut ObjectHeap, object: &Value, property: &str) -> Result<bool> {
    match object {
        Value::Object(id) => objects.delete(*id, property),
        Value::Undefined | Value::Null => Err(Error::runtime(NULLISH_PROPERTY_DELETE_ERROR)),
        Value::Error(_)
        | Value::Bool(_)
        | Value::Number(_)
        | Value::String(_)
        | Value::Function(_) => Ok(true),
    }
}
