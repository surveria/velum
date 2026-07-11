use crate::error::{Error, JavaScriptErrorMetadata, Result};
use crate::storage::symbol::JsSymbol;
use crate::value::{ObjectId, Value};

mod accounting;
mod array;
mod base;
mod data;
mod date;
mod heap;
mod integrity;
mod property;
mod prototype;
mod proxy;
mod regexp;
mod shape;
mod string;
mod trace;
mod typed_array;

use array::{ArrayIndex, ArrayLength, ArrayStorage};
use base::LiteralPrototype;
pub use base::ObjectHeap;
pub use date::DateValue;
pub(in crate::runtime) use property::AccessorPropertyDescriptor;
use property::NamedProperty;
pub use property::ObjectPropertyInit;
pub use property::{
    AccessorPropertyUpdate, CacheableNativePropertyValue, CacheablePropertyDelete,
    CacheablePropertyLookup, CacheablePropertyPresence, CacheablePropertyValue,
    CacheablePropertyWrite, DataPropertyDescriptor, DataPropertyUpdate, ObjectProperty,
    OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable, PropertyKey, PropertyLookup,
    PropertyUpdate, PropertyWritable,
};
pub use proxy::ProxyValue;
pub use regexp::RegExpValue;
use shape::{ShapeId, ShapeTable};
pub use typed_array::{ByteBuffer, ByteBufferOrigin};
pub(in crate::runtime) use typed_array::{
    TypedArrayElementKind, TypedArrayView, typed_array_number,
};

const ARRAY_LENGTH_PROPERTY: &str = "length";
const ARRAY_INDEX_LIMIT_ERROR: &str = "array index exceeded supported range";
pub const OBJECT_CONSTRUCTOR_PROPERTY: &str = "constructor";
const PROTOTYPE_PROPERTY: &str = "__proto__";

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectPropertyValue {
    Value(Value),
    StringCharacter(char),
    /// An accessor property was found; the payload is its getter function,
    /// which the caller must invoke with the original receiver as `this`.
    Getter(Value),
}

impl ObjectPropertyValue {
    const fn value(value: Value) -> Self {
        Self::Value(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::runtime) enum ObjectPrimitiveValue {
    Bool(bool),
    Number(f64),
    Symbol(JsSymbol),
}

/// How an assignment should proceed after resolving accessor properties on
/// the receiver and its prototype chain.
#[derive(Debug, Clone)]
pub enum AccessorWriteDisposition {
    /// No accessor property found; ordinary data-write semantics apply.
    None,
    /// An accessor with a setter intercepts the write.
    Setter(Value),
    /// A getter-only accessor swallows the write (sloppy-mode no-op).
    NoSetter,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
enum ObjectExtensibility {
    #[default]
    Extensible,
    NonExtensible,
}

impl ObjectExtensibility {
    const fn is_extensible(self) -> bool {
        matches!(self, Self::Extensible)
    }
}

#[derive(Debug, Clone)]
struct Object {
    named_properties: Vec<NamedProperty>,
    array_storage: ArrayStorage,
    shape: ShapeId,
    enumerable_property_count: usize,
    array_length: Option<ArrayLength>,
    array_length_writable: PropertyWritable,
    string_value: Option<crate::storage::string_heap::JsString>,
    primitive_value: Option<ObjectPrimitiveValue>,
    error_metadata: Option<JavaScriptErrorMetadata>,
    date_value: Option<DateValue>,
    regexp_value: Option<RegExpValue>,
    proxy_value: Option<ProxyValue>,
    byte_buffer: Option<ByteBuffer>,
    typed_array: Option<TypedArrayView>,
    is_raw_json: bool,
    prototype: Option<ObjectId>,
    extensibility: ObjectExtensibility,
    storage_ledger: Option<crate::runtime::storage_ledger::VmStorageLedger>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ObjectStructureSnapshot {
    shape: ShapeId,
    property_count: usize,
    enumerable_property_count: usize,
    prototype: Option<ObjectId>,
    extensibility: ObjectExtensibility,
    array_length_writable: PropertyWritable,
}

impl Object {
    const fn ordinary() -> Self {
        Self {
            named_properties: Vec::new(),
            array_storage: ArrayStorage::new(),
            shape: ShapeId::root(),
            enumerable_property_count: 0,
            array_length: None,
            array_length_writable: PropertyWritable::Yes,
            string_value: None,
            primitive_value: None,
            error_metadata: None,
            date_value: None,
            regexp_value: None,
            proxy_value: None,
            byte_buffer: None,
            typed_array: None,
            is_raw_json: false,
            prototype: None,
            extensibility: ObjectExtensibility::Extensible,
            storage_ledger: None,
        }
    }

    fn activate_storage(
        &mut self,
        storage_ledger: crate::runtime::storage_ledger::VmStorageLedger,
    ) -> Result<()> {
        if self.storage_ledger.is_some() {
            return Err(Error::runtime("object property storage is already active"));
        }
        let reservation = storage_ledger.reserve_count(
            crate::runtime::VmStorageKind::ObjectProperty,
            self.property_count(),
        )?;
        reservation.commit()?;
        self.storage_ledger = Some(storage_ledger);
        Ok(())
    }

    fn reserve_property_growth(
        &self,
    ) -> Result<Option<crate::runtime::storage_ledger::VmStorageReservation>> {
        self.reserve_property_growth_by(1)
    }

    fn reserve_property_growth_by(
        &self,
        additional_count: usize,
    ) -> Result<Option<crate::runtime::storage_ledger::VmStorageReservation>> {
        self.storage_ledger
            .as_ref()
            .map(|storage_ledger| {
                storage_ledger.reserve_count(
                    crate::runtime::VmStorageKind::ObjectProperty,
                    additional_count,
                )
            })
            .transpose()
    }

    fn release_property(&self) -> Result<()> {
        let Some(storage_ledger) = &self.storage_ledger else {
            return Ok(());
        };
        storage_ledger.release_count(crate::runtime::VmStorageKind::ObjectProperty, 1)
    }

    fn ordinary_with_property_capacity(capacity: usize) -> Self {
        Self {
            named_properties: Vec::with_capacity(capacity),
            array_storage: ArrayStorage::new(),
            shape: ShapeId::root(),
            enumerable_property_count: 0,
            array_length: None,
            array_length_writable: PropertyWritable::Yes,
            string_value: None,
            primitive_value: None,
            error_metadata: None,
            date_value: None,
            regexp_value: None,
            proxy_value: None,
            byte_buffer: None,
            typed_array: None,
            is_raw_json: false,
            prototype: None,
            extensibility: ObjectExtensibility::Extensible,
            storage_ledger: None,
        }
    }

    const fn array(length: ArrayLength) -> Self {
        Self {
            named_properties: Vec::new(),
            array_storage: ArrayStorage::new(),
            shape: ShapeId::root(),
            enumerable_property_count: 0,
            array_length: Some(length),
            array_length_writable: PropertyWritable::Yes,
            string_value: None,
            primitive_value: None,
            error_metadata: None,
            date_value: None,
            regexp_value: None,
            proxy_value: None,
            byte_buffer: None,
            typed_array: None,
            is_raw_json: false,
            prototype: None,
            extensibility: ObjectExtensibility::Extensible,
            storage_ledger: None,
        }
    }

    const fn boxed_primitive(value: ObjectPrimitiveValue) -> Self {
        Self {
            named_properties: Vec::new(),
            array_storage: ArrayStorage::new(),
            shape: ShapeId::root(),
            enumerable_property_count: 0,
            array_length: None,
            array_length_writable: PropertyWritable::Yes,
            string_value: None,
            primitive_value: Some(value),
            error_metadata: None,
            date_value: None,
            regexp_value: None,
            proxy_value: None,
            byte_buffer: None,
            typed_array: None,
            is_raw_json: false,
            prototype: None,
            extensibility: ObjectExtensibility::Extensible,
            storage_ledger: None,
        }
    }

    const fn literal_prototype(value: &Value) -> Option<LiteralPrototype> {
        match value {
            Value::Object(prototype) => Some(LiteralPrototype::Object(*prototype)),
            Value::Null => Some(LiteralPrototype::Null),
            Value::Undefined
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => None,
        }
    }

    const fn structure_snapshot(&self) -> ObjectStructureSnapshot {
        ObjectStructureSnapshot {
            shape: self.shape,
            property_count: self.property_count(),
            enumerable_property_count: self.enumerable_property_count,
            prototype: self.prototype,
            extensibility: self.extensibility,
            array_length_writable: self.array_length_writable,
        }
    }

    fn get_own(
        &self,
        property: PropertyLookup<'_>,
        shapes: &ShapeTable,
    ) -> Result<Option<ObjectPropertyValue>> {
        if let Some(ch) = self.virtual_string_character(property.name())? {
            return Ok(Some(ObjectPropertyValue::StringCharacter(ch)));
        }
        if let Some(length) = self
            .array_length
            .filter(|_| property.name() == ARRAY_LENGTH_PROPERTY)
        {
            return Ok(Some(ObjectPropertyValue::value(length.value())));
        }
        if let Some(value) = self.typed_array_property(property.name())? {
            return Ok(Some(ObjectPropertyValue::value(value)));
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property.name())
            && let Some(value) = self.array_element_value(index)
        {
            return Ok(Some(ObjectPropertyValue::value(value)));
        }
        let Some(key) = property.key() else {
            return Ok(None);
        };
        let Some(named) = self.named_property(shapes, key)? else {
            return Ok(None);
        };
        if let Some(accessor) = named.accessor() {
            if accessor.has_getter() {
                return Ok(Some(ObjectPropertyValue::Getter(accessor.get())));
            }
            return Ok(Some(ObjectPropertyValue::value(Value::Undefined)));
        }
        Ok(Some(ObjectPropertyValue::value(named.value())))
    }

    fn has_own(&self, property: PropertyLookup<'_>, shapes: &ShapeTable) -> Result<bool> {
        if self.has_virtual_string_property(property)? {
            return Ok(true);
        }
        if self.array_length.is_some() && property.name() == ARRAY_LENGTH_PROPERTY {
            return Ok(true);
        }
        if self.has_typed_array_property(property.name())? {
            return Ok(true);
        }
        if self.array_length.is_some()
            && ArrayIndex::parse(property.name()).is_some_and(|index| self.has_array_element(index))
        {
            return Ok(true);
        }
        let Some(key) = property.key() else {
            return Ok(false);
        };
        self.named_property(shapes, key)
            .map(|property| property.is_some())
    }

    fn set(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        if self.array_length.is_some() && property_name == ARRAY_LENGTH_PROPERTY {
            return Err(Error::runtime("array length assignment is not supported"));
        }
        let index = ArrayIndex::parse(property_name);
        if let Some(index) = index
            && self.set_typed_array_index(index, &value)?
        {
            return Ok(());
        }
        if self.has_virtual_string_property_name(property_name)? {
            return Ok(());
        }
        self.set_ordinary(property, property_name, value, shapes, max_properties)?;
        if let Some(index) = index {
            self.extend_array_length(index)?;
        }
        Ok(())
    }

    fn set_ordinary(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        self.set_property_value(property, property_name, value, None, shapes, max_properties)
    }

    fn define(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        enumerable: PropertyEnumerable,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        self.set_property_value(
            property,
            property_name,
            value,
            Some(enumerable),
            shapes,
            max_properties,
        )
    }

    fn set_property_value(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        let index = ArrayIndex::parse(property_name);
        if self.array_length.is_some()
            && let Some(index) = index
        {
            self.set_array_property_value(
                index,
                Some((property, property_name)),
                value,
                enumerable,
                Some(shapes),
                max_properties,
            )?;
            return self.extend_array_length(index);
        }

        self.set_named_property_value(property, value, enumerable, shapes, max_properties)?;
        if let Some(index) = index {
            self.array_storage.insert_sparse_key(index, property);
        }
        Ok(())
    }

    fn set_named_property_value(
        &mut self,
        property: PropertyKey,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        let property_count = self.property_count();
        let enumerable_update = if self.contains_named_property(shapes, property)? {
            let (was_enumerable, is_enumerable, attributes) = {
                let existing = self.named_property_mut(shapes, property)?;
                let was_enumerable = existing.is_enumerable();
                existing.set_value(value);
                if let Some(enumerable) = enumerable {
                    existing.set_enumerable(enumerable);
                }
                (
                    was_enumerable,
                    existing.is_enumerable(),
                    existing.shape_attributes(),
                )
            };
            self.shape = shapes.transition_after_update(self.shape, property, attributes)?;
            Some((was_enumerable, is_enumerable))
        } else {
            if !self.extensibility.is_extensible() {
                return Ok(());
            }
            if property_count >= max_properties {
                return Err(Error::limit(format!(
                    "object property count exceeded {max_properties}"
                )));
            }
            let named_property =
                ObjectProperty::ordinary(value, enumerable.unwrap_or(PropertyEnumerable::Yes));
            let enumerable_update = named_property.is_enumerable().then_some((false, true));
            self.push_named_property(shapes, property, named_property)?;
            enumerable_update
        };
        if let Some((was_enumerable, is_enumerable)) = enumerable_update {
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
        }
        Ok(())
    }

    fn set_array_property_value(
        &mut self,
        index: ArrayIndex,
        property: Option<(PropertyKey, &str)>,
        value: Value,
        enumerable: Option<PropertyEnumerable>,
        shapes: Option<&mut ShapeTable>,
        max_properties: usize,
    ) -> Result<()> {
        if index.dense_position(max_properties)?.is_none() {
            let Some((property, _)) = property else {
                return Err(Error::runtime("sparse array property key is not available"));
            };
            let Some(shapes) = shapes else {
                return Err(Error::runtime("sparse array shape table is not available"));
            };
            if !self.extensibility.is_extensible() {
                return Ok(());
            }
            self.array_storage.insert_sparse_key(index, property);
            return self.set_named_property_value(
                property,
                value,
                enumerable,
                shapes,
                max_properties,
            );
        }

        if let Some(property) = self.array_storage.dense_property_mut(index)? {
            let was_enumerable = property.is_enumerable();
            property.set_value(value);
            if let Some(enumerable) = enumerable {
                property.set_enumerable(enumerable);
            }
            let is_enumerable = property.is_enumerable();
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
            return Ok(());
        }

        if !self.extensibility.is_extensible() {
            return Ok(());
        }
        if self.property_count() >= max_properties {
            return Err(Error::limit(format!(
                "object property count exceeded {max_properties}"
            )));
        }
        let property =
            ObjectProperty::ordinary(value, enumerable.unwrap_or(PropertyEnumerable::Yes));
        let is_enumerable = property.is_enumerable();
        let reservation = self.reserve_property_growth()?;
        if let Some(reservation) = reservation {
            reservation.commit()?;
        }
        let previous = match self.array_storage.insert_dense_property(index, property) {
            Ok(previous) => previous,
            Err(error) => {
                self.release_property()?;
                return Err(error);
            }
        };
        if previous.is_some() {
            self.release_property()?;
            return Err(Error::runtime("array index storage replaced existing slot"));
        }
        if is_enumerable {
            self.enumerable_property_count = self.enumerable_property_count.saturating_add(1);
        }
        Ok(())
    }

    fn delete(&mut self, property: PropertyLookup<'_>, shapes: &mut ShapeTable) -> Result<bool> {
        if self.has_virtual_string_property(property)? {
            return Ok(false);
        }
        if self.array_length.is_some() && property.name() == ARRAY_LENGTH_PROPERTY {
            return Ok(false);
        }
        if self.has_typed_array_property(property.name())? {
            return Ok(false);
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property.name())
            && self.delete_array_element(index)?
        {
            return Ok(true);
        }
        let Some(key) = property.key() else {
            return Ok(true);
        };
        let Some(existing_property) = self.named_property(shapes, key)? else {
            return Ok(true);
        };
        if !existing_property.is_configurable() {
            return Ok(false);
        }
        let Some(removed_property) = self.remove_named_property(shapes, key)? else {
            return Ok(true);
        };
        if removed_property.is_enumerable() {
            self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
        }
        if let Some(index) = ArrayIndex::parse(property.name()) {
            self.array_storage.remove_sparse_key(index);
        }
        Ok(true)
    }

    fn extend_array_length(&mut self, index: ArrayIndex) -> Result<()> {
        let Some(length) = self.array_length else {
            return Ok(());
        };
        if length.contains(index) {
            return Ok(());
        }
        self.array_length = Some(index.next_length()?);
        Ok(())
    }

    fn array_element_value(&self, index: ArrayIndex) -> Option<Value> {
        let position = index.position().ok()?;
        self.array_storage
            .dense_property_at_position(position)
            .map(ObjectProperty::value)
    }

    fn has_array_element(&self, index: ArrayIndex) -> bool {
        self.array_storage.dense_property(index).is_some()
    }

    fn typed_array_property(&self, property: &str) -> Result<Option<Value>> {
        if let Some(buffer) = self.byte_buffer.as_ref()
            && property == "byteLength"
        {
            return Ok(Some(Value::Number(usize_property_number(
                buffer.byte_length(),
            )?)));
        }
        let Some(view) = self.typed_array.as_ref() else {
            return Ok(None);
        };
        match property {
            "length" => Ok(Some(Value::Number(usize_property_number(view.length())?))),
            "byteLength" => Ok(Some(Value::Number(usize_property_number(
                view.byte_length()?,
            )?))),
            "byteOffset" => Ok(Some(Value::Number(usize_property_number(
                view.byte_offset(),
            )?))),
            "buffer" => Ok(Some(Value::Object(view.buffer_object()))),
            _ => {
                let Some(index) = ArrayIndex::parse(property) else {
                    return Ok(None);
                };
                let Some(number) = view.read(index.position()?)? else {
                    return Ok(None);
                };
                Ok(Some(Value::Number(number)))
            }
        }
    }

    fn has_typed_array_property(&self, property: &str) -> Result<bool> {
        if self.byte_buffer.is_some() && property == "byteLength" {
            return Ok(true);
        }
        let Some(view) = self.typed_array.as_ref() else {
            return Ok(false);
        };
        if matches!(property, "length" | "byteLength" | "byteOffset" | "buffer") {
            return Ok(true);
        }
        let Some(index) = ArrayIndex::parse(property) else {
            return Ok(false);
        };
        Ok(index.position()? < view.length())
    }

    fn set_typed_array_index(&self, index: ArrayIndex, value: &Value) -> Result<bool> {
        let Some(view) = self.typed_array.as_ref() else {
            return Ok(false);
        };
        view.write(index.position()?, typed_array_number(value)?)
    }

    fn delete_array_element(&mut self, index: ArrayIndex) -> Result<bool> {
        let Some(property) = self.array_storage.dense_property(index) else {
            return Ok(false);
        };
        if !property.is_configurable() {
            return Ok(false);
        }
        if let Ok(Some(property)) = self.array_storage.remove_dense_property(index) {
            if property.is_enumerable() {
                self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
            }
            self.release_property()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn has_enumerable_own_keys(&self) -> bool {
        self.enumerable_property_count > 0 || self.has_virtual_string_keys()
    }

    const fn update_enumerable_property_count(
        &mut self,
        was_enumerable: bool,
        is_enumerable: bool,
    ) {
        match (was_enumerable, is_enumerable) {
            (false, true) => {
                self.enumerable_property_count = self.enumerable_property_count.saturating_add(1);
            }
            (true, false) => {
                self.enumerable_property_count = self.enumerable_property_count.saturating_sub(1);
            }
            (true, true) | (false, false) => {}
        }
    }

    const fn property_count(&self) -> usize {
        self.named_properties
            .len()
            .saturating_add(self.array_storage.property_count())
    }
}

fn usize_property_number(value: usize) -> Result<f64> {
    let value = u32::try_from(value)
        .map_err(|_| Error::limit("typed array length exceeded supported number range"))?;
    Ok(f64::from(value))
}
