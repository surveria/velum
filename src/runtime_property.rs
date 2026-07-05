use crate::error::{Error, Result};
use crate::runtime_assertions::error_property;
use crate::runtime_object::ObjectHeap;
use crate::value::Value;

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
