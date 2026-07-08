use crate::error::{Error, Result};
use crate::runtime::control::error_property_text;
use crate::runtime::object::{ObjectHeap, ObjectPropertyValue, PropertyKey, PropertyLookup};
use crate::storage::atom::AtomTable;
use crate::value::Value;

mod accessor;
mod dynamic;
pub mod static_names;
pub mod well_known;

const NULLISH_PROPERTY_DELETE_ERROR: &str = "Cannot convert undefined or null to object";
const ERROR_NAME_PROPERTY: &str = "name";
const ERROR_MESSAGE_PROPERTY: &str = "message";
const STRING_LENGTH_PROPERTY: &str = "length";

pub fn property_key(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::HeapString(value) => value.as_str().to_owned(),
        Value::Symbol(value) => value.display_name(),
        _ => value.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct DynamicPropertyKey {
    name: String,
    key: Option<PropertyKey>,
}

impl DynamicPropertyKey {
    pub(crate) const fn new(name: String, key: Option<PropertyKey>) -> Self {
        Self { name, key }
    }

    pub(crate) const fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) const fn key(&self) -> Option<PropertyKey> {
        self.key
    }

    pub(crate) const fn lookup(&self) -> PropertyLookup<'_> {
        if let Some(key) = self.key {
            return PropertyLookup::from_key(self.name(), key);
        }
        PropertyLookup::new(self.name(), None)
    }

    pub(crate) const fn remember_key(&mut self, key: PropertyKey) {
        self.key = Some(key);
    }
}

pub fn get_property<'a>(
    objects: &ObjectHeap,
    object: &'a Value,
    property: PropertyLookup<'_>,
) -> Result<PropertyValue<'a>> {
    match object {
        Value::Error(error) => Ok(error_property_value(error, property.name())),
        Value::Object(id) => objects
            .get(*id, property)
            .map(|value| PropertyValue::from_object_value(value, object)),
        Value::String(value) => string_property(value, property.name()),
        Value::HeapString(value) => string_property(value.as_str(), property.name()),
        value => Err(Error::runtime(format!(
            "member access '{}' is not supported for {}",
            property.name(),
            value.type_name()
        ))),
    }
}

#[derive(Debug, Clone)]
pub enum PropertyValue<'a> {
    Value(Value),
    Text(&'a str),
    Character(char),
    /// The property is an accessor; the caller must invoke `getter` with
    /// `receiver` as `this` to obtain the value.
    Getter {
        getter: Value,
        receiver: Value,
    },
}

impl PropertyValue<'_> {
    fn from_object_value(value: ObjectPropertyValue, receiver: &Value) -> Self {
        match value {
            ObjectPropertyValue::Value(value) => Self::Value(value),
            ObjectPropertyValue::StringCharacter(ch) => Self::Character(ch),
            ObjectPropertyValue::Getter(getter) => Self::Getter {
                getter,
                receiver: receiver.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StringPropertyValue {
    Length(f64),
    Character(char),
    Missing,
}

pub fn has_property(
    objects: &ObjectHeap,
    object: &Value,
    property: PropertyLookup<'_>,
) -> Result<bool> {
    match object {
        Value::Error(_) => Ok(matches!(
            property.name(),
            ERROR_NAME_PROPERTY | ERROR_MESSAGE_PROPERTY
        )),
        Value::Object(id) => objects.has(*id, property),
        Value::String(value) => string_has_property(value, property.name()),
        Value::HeapString(value) => string_has_property(value.as_str(), property.name()),
        value => Err(Error::runtime(format!(
            "operator 'in' is not supported for {}",
            value.type_name()
        ))),
    }
}

pub fn enumerable_property_keys(
    objects: &ObjectHeap,
    atoms: &AtomTable,
    object: &Value,
) -> Result<Vec<String>> {
    match object {
        Value::Undefined | Value::Null => Err(Error::runtime(NULLISH_PROPERTY_DELETE_ERROR)),
        Value::Object(id) => objects.keys(*id, atoms),
        Value::Error(_) => Ok(vec![
            ERROR_NAME_PROPERTY.to_owned(),
            ERROR_MESSAGE_PROPERTY.to_owned(),
        ]),
        Value::String(value) => string_enumerable_keys(value),
        Value::HeapString(value) => string_enumerable_keys(value.as_str()),
        Value::Bool(_)
        | Value::Number(_)
        | Value::Symbol(_)
        | Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_) => Ok(Vec::new()),
    }
}

pub fn set_property(
    objects: &mut ObjectHeap,
    object: &Value,
    property: PropertyKey,
    property_name: &str,
    value: Value,
    max_properties: usize,
) -> Result<()> {
    let Value::Object(id) = object else {
        return Err(Error::runtime(format!(
            "property assignment '{property_name}' is not supported for {}",
            object.type_name()
        )));
    };
    objects.set(*id, property, property_name, value, max_properties)
}

pub fn delete_property(
    objects: &mut ObjectHeap,
    object: &Value,
    property: PropertyLookup<'_>,
) -> Result<bool> {
    match object {
        Value::Object(id) => objects.delete(*id, property),
        Value::Undefined | Value::Null => Err(Error::runtime(NULLISH_PROPERTY_DELETE_ERROR)),
        Value::Error(_)
        | Value::Bool(_)
        | Value::Number(_)
        | Value::String(_)
        | Value::HeapString(_)
        | Value::Symbol(_)
        | Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_) => Ok(true),
    }
}

fn error_property_value<'a>(
    error: &'a crate::value::ErrorObject,
    property: &str,
) -> PropertyValue<'a> {
    error_property_text(error, property)
        .map_or(PropertyValue::Value(Value::Undefined), PropertyValue::Text)
}

fn string_property<'a>(value: &str, property: &str) -> Result<PropertyValue<'a>> {
    match string_property_value(value, property)? {
        StringPropertyValue::Length(value) => Ok(PropertyValue::Value(Value::Number(value))),
        StringPropertyValue::Character(ch) => Ok(PropertyValue::Character(ch)),
        StringPropertyValue::Missing => Ok(PropertyValue::Value(Value::Undefined)),
    }
}

pub fn string_property_value(value: &str, property: &str) -> Result<StringPropertyValue> {
    if property == STRING_LENGTH_PROPERTY {
        return string_length(value).map(StringPropertyValue::Length);
    }
    Ok(string_index_value(value, property)
        .map_or(StringPropertyValue::Missing, StringPropertyValue::Character))
}

fn string_has_property(value: &str, property: &str) -> Result<bool> {
    if property == STRING_LENGTH_PROPERTY {
        return Ok(true);
    }
    let Some(index) = string_property_index(property) else {
        return Ok(false);
    };
    Ok(index < string_len(value)?)
}

fn string_enumerable_keys(value: &str) -> Result<Vec<String>> {
    let len = string_len(value)?;
    let mut keys = Vec::with_capacity(len);
    for index in 0..len {
        keys.push(index.to_string());
    }
    Ok(keys)
}

fn string_index_value(value: &str, property: &str) -> Option<char> {
    let index = string_property_index(property)?;
    value.chars().nth(index)
}

fn string_property_index(property: &str) -> Option<usize> {
    let index = property.parse::<usize>().ok()?;
    if index.to_string() == property {
        return Some(index);
    }
    None
}

fn string_length(value: &str) -> Result<f64> {
    let len = u32::try_from(string_len(value)?)
        .map_err(|_| Error::limit("string length exceeded supported property range"))?;
    Ok(f64::from(len))
}

fn string_len(value: &str) -> Result<usize> {
    let len = value.chars().count();
    u32::try_from(len)
        .map_err(|_| Error::limit("string length exceeded supported property range"))
        .map(|_| len)
}
