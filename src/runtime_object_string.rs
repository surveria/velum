use crate::{
    error::{Error, Result},
    value::{ObjectId, Value},
};

use super::{ArrayIndex, Object, ObjectHeap, PropertyEnumerable};

const STRING_LENGTH_PROPERTY: &str = "length";
const STRING_LENGTH_LIMIT_ERROR: &str = "string length exceeded supported object range";

impl ObjectHeap {
    pub(crate) fn create_string_object(
        &mut self,
        value: &str,
        prototype: ObjectId,
        max_objects: usize,
        max_properties: usize,
    ) -> Result<Value> {
        let length = string_character_count(value)?;
        let property_capacity = length
            .checked_add(1)
            .ok_or_else(|| Error::limit(STRING_LENGTH_LIMIT_ERROR))?;
        let mut object = Object::ordinary_with_property_capacity(property_capacity);
        object.prototype = Some(prototype);
        object.define(
            STRING_LENGTH_PROPERTY.to_owned(),
            Value::Number(length_to_value(length)?),
            PropertyEnumerable::No,
            max_properties,
        )?;
        for (index, ch) in value.chars().enumerate() {
            let index = ArrayIndex::from_usize(index)?;
            object.define(
                index.key(),
                Value::String(ch.to_string()),
                PropertyEnumerable::Yes,
                max_properties,
            )?;
        }
        self.push_object(object, max_objects).map(Value::Object)
    }
}

fn string_character_count(value: &str) -> Result<usize> {
    let count = value.chars().count();
    u32::try_from(count)
        .map_err(|_| Error::limit(STRING_LENGTH_LIMIT_ERROR))
        .map(|_| count)
}

fn length_to_value(length: usize) -> Result<f64> {
    let length = u32::try_from(length).map_err(|_| Error::limit(STRING_LENGTH_LIMIT_ERROR))?;
    Ok(f64::from(length))
}
