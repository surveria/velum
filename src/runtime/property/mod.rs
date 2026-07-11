use crate::error::{Error, Result};
use crate::runtime::object::{ObjectHeap, ObjectPropertyValue, PropertyKey, PropertyLookup};
use crate::storage::atom::AtomTable;
use crate::value::{ObjectId, Value};

mod accessor;
mod dynamic;
pub mod static_names;
pub mod well_known;

const NULLISH_PROPERTY_DELETE_ERROR: &str = "Cannot convert undefined or null to object";
const STRING_LENGTH_PROPERTY: &str = "length";

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

pub fn get_property(
    objects: &ObjectHeap,
    object: &Value,
    property: PropertyLookup<'_>,
) -> Result<PropertyValue> {
    match object {
        Value::Object(id) => objects
            .get(*id, property)
            .map(|value| PropertyValue::from_object_value(value, object)),
        Value::String(value) => string_property(value, property.name()),
        Value::HeapString(value) => utf16_string_property(value.as_utf16(), property.name()),
        value => Err(Error::type_error(format!(
            "member access '{}' is not supported for {}",
            property.name(),
            value.type_name()
        ))),
    }
}

pub fn get_property_with_receiver(
    objects: &ObjectHeap,
    object: ObjectId,
    receiver: &Value,
    property: PropertyLookup<'_>,
) -> Result<PropertyValue> {
    objects
        .get(object, property)
        .map(|value| PropertyValue::from_object_value(value, receiver))
}

#[derive(Debug, Clone)]
pub enum PropertyValue {
    Value(Value),
    CodeUnit(u16),
    /// The property is an accessor; the caller must invoke `getter` with
    /// `receiver` as `this` to obtain the value.
    Getter {
        getter: Value,
        receiver: Value,
    },
}

impl PropertyValue {
    fn from_object_value(value: ObjectPropertyValue, receiver: &Value) -> Self {
        match value {
            ObjectPropertyValue::Value(value) => Self::Value(value),
            ObjectPropertyValue::StringCodeUnit(unit) => Self::CodeUnit(unit),
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
    CodeUnit(u16),
    Missing,
}

pub fn has_property(
    objects: &ObjectHeap,
    object: &Value,
    property: PropertyLookup<'_>,
) -> Result<bool> {
    match object {
        Value::Object(id) => objects.has(*id, property),
        Value::String(value) => string_has_property(value, property.name()),
        Value::HeapString(value) => utf16_string_has_property(value.as_utf16(), property.name()),
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
        Value::String(value) => string_enumerable_keys(value),
        Value::HeapString(value) => utf16_string_enumerable_keys(value.as_utf16()),
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
        Value::Bool(_)
        | Value::Number(_)
        | Value::String(_)
        | Value::HeapString(_)
        | Value::Symbol(_)
        | Value::Function(_)
        | Value::NativeFunction(_)
        | Value::HostFunction(_) => Ok(true),
    }
}

fn string_property(value: &str, property: &str) -> Result<PropertyValue> {
    let units = value.encode_utf16().collect::<Vec<_>>();
    utf16_string_property(&units, property)
}

fn utf16_string_property(value: &[u16], property: &str) -> Result<PropertyValue> {
    match utf16_string_property_value(value, property)? {
        StringPropertyValue::Length(value) => Ok(PropertyValue::Value(Value::Number(value))),
        StringPropertyValue::CodeUnit(unit) => Ok(PropertyValue::CodeUnit(unit)),
        StringPropertyValue::Missing => Ok(PropertyValue::Value(Value::Undefined)),
    }
}

pub fn string_property_value(value: &str, property: &str) -> Result<StringPropertyValue> {
    let units = value.encode_utf16().collect::<Vec<_>>();
    utf16_string_property_value(&units, property)
}

pub fn utf16_string_property_value(value: &[u16], property: &str) -> Result<StringPropertyValue> {
    if property == STRING_LENGTH_PROPERTY {
        return string_length(value).map(StringPropertyValue::Length);
    }
    Ok(string_index_value(value, property)
        .map_or(StringPropertyValue::Missing, StringPropertyValue::CodeUnit))
}

pub(in crate::runtime) fn string_length_value_if_string(
    object: &Value,
    property: &str,
) -> Result<Option<Value>> {
    if property != STRING_LENGTH_PROPERTY {
        return Ok(None);
    }
    match object {
        Value::String(value) => {
            let units = value.encode_utf16().collect::<Vec<_>>();
            string_length(&units).map(Value::Number).map(Some)
        }
        Value::HeapString(value) => string_length(value.as_utf16()).map(Value::Number).map(Some),
        _ => Ok(None),
    }
}

fn string_has_property(value: &str, property: &str) -> Result<bool> {
    let units = value.encode_utf16().collect::<Vec<_>>();
    utf16_string_has_property(&units, property)
}

fn utf16_string_has_property(value: &[u16], property: &str) -> Result<bool> {
    if property == STRING_LENGTH_PROPERTY {
        return Ok(true);
    }
    let Some(index) = string_property_index(property) else {
        return Ok(false);
    };
    Ok(index < string_len(value)?)
}

fn string_enumerable_keys(value: &str) -> Result<Vec<String>> {
    let units = value.encode_utf16().collect::<Vec<_>>();
    utf16_string_enumerable_keys(&units)
}

fn utf16_string_enumerable_keys(value: &[u16]) -> Result<Vec<String>> {
    let len = string_len(value)?;
    let mut keys = Vec::with_capacity(len);
    for index in 0..len {
        keys.push(index.to_string());
    }
    Ok(keys)
}

fn string_index_value(value: &[u16], property: &str) -> Option<u16> {
    let index = string_property_index(property)?;
    value.get(index).copied()
}

fn string_property_index(property: &str) -> Option<usize> {
    let index = property.parse::<usize>().ok()?;
    if index.to_string() == property {
        return Some(index);
    }
    None
}

fn string_length(value: &[u16]) -> Result<f64> {
    let len = u32::try_from(string_len(value)?)
        .map_err(|_| Error::limit("string length exceeded supported property range"))?;
    Ok(f64::from(len))
}

fn string_len(value: &[u16]) -> Result<usize> {
    let len = value.len();
    u32::try_from(len)
        .map_err(|_| Error::limit("string length exceeded supported property range"))
        .map(|_| len)
}
