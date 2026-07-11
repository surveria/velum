use crate::{
    error::Result,
    runtime::trace::{StrongEdgeReference, StrongEdgeVisitor},
    value::{ObjectId, Value},
};

use super::{
    ARRAY_LENGTH_PROPERTY, ArrayIndex, Object, ObjectHeap, PropertyKey, PropertyLookup,
    ShapePropertyAttributes, ShapeTable,
};

mod validation;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PropertyEnumerable {
    Yes,
    No,
}

impl PropertyEnumerable {
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PropertyWritable {
    Yes,
    No,
}

impl PropertyWritable {
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PropertyConfigurable {
    Yes,
    No,
}

impl PropertyConfigurable {
    pub const fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

#[derive(Debug, Clone)]
pub struct DataPropertyDescriptor {
    value: Value,
    writable: PropertyWritable,
    enumerable: PropertyEnumerable,
    configurable: PropertyConfigurable,
}

impl DataPropertyDescriptor {
    pub const fn new(
        value: Value,
        writable: PropertyWritable,
        enumerable: PropertyEnumerable,
        configurable: PropertyConfigurable,
    ) -> Self {
        Self {
            value,
            writable,
            enumerable,
            configurable,
        }
    }

    pub fn value(&self) -> Value {
        self.value.clone()
    }

    pub const fn value_ref(&self) -> &Value {
        &self.value
    }

    pub const fn writable(&self) -> PropertyWritable {
        self.writable
    }

    pub const fn enumerable(&self) -> PropertyEnumerable {
        self.enumerable
    }

    pub const fn configurable(&self) -> PropertyConfigurable {
        self.configurable
    }
}

#[derive(Debug, Clone)]
pub struct DataPropertyUpdate {
    value: Option<Value>,
    writable: Option<PropertyWritable>,
    enumerable: Option<PropertyEnumerable>,
    configurable: Option<PropertyConfigurable>,
}

impl DataPropertyUpdate {
    pub const fn new(
        value: Option<Value>,
        writable: Option<PropertyWritable>,
        enumerable: Option<PropertyEnumerable>,
        configurable: Option<PropertyConfigurable>,
    ) -> Self {
        Self {
            value,
            writable,
            enumerable,
            configurable,
        }
    }

    pub fn value(&self) -> Option<Value> {
        self.value.clone()
    }

    pub(in crate::runtime) const fn trace_values(&self) -> [Option<&Value>; 2] {
        [self.value.as_ref(), None]
    }

    pub const fn writable(&self) -> Option<PropertyWritable> {
        self.writable
    }

    pub const fn enumerable(&self) -> Option<PropertyEnumerable> {
        self.enumerable
    }

    pub const fn configurable(&self) -> Option<PropertyConfigurable> {
        self.configurable
    }

    pub(in crate::runtime) fn replace_value(&mut self, value: Value) {
        self.value = Some(value);
    }

    const fn is_generic(&self) -> bool {
        self.value.is_none() && self.writable.is_none()
    }

    pub fn complete_for_new(self) -> DataPropertyDescriptor {
        DataPropertyDescriptor::new(
            self.value.unwrap_or(Value::Undefined),
            self.writable.unwrap_or(PropertyWritable::No),
            self.enumerable.unwrap_or(PropertyEnumerable::No),
            self.configurable.unwrap_or(PropertyConfigurable::No),
        )
    }
}

/// Accessor property state: getter/setter function values (`Value::Undefined`
/// marks an absent half) plus the shared attribute pair.
#[derive(Debug, Clone)]
pub struct AccessorPropertyDescriptor {
    get: Value,
    set: Value,
    enumerable: PropertyEnumerable,
    configurable: PropertyConfigurable,
}

impl AccessorPropertyDescriptor {
    pub const fn new(
        get: Value,
        set: Value,
        enumerable: PropertyEnumerable,
        configurable: PropertyConfigurable,
    ) -> Self {
        Self {
            get,
            set,
            enumerable,
            configurable,
        }
    }

    pub fn get(&self) -> Value {
        self.get.clone()
    }

    pub fn set(&self) -> Value {
        self.set.clone()
    }

    pub const fn get_ref(&self) -> &Value {
        &self.get
    }

    pub const fn set_ref(&self) -> &Value {
        &self.set
    }

    pub const fn has_getter(&self) -> bool {
        !matches!(self.get, Value::Undefined)
    }

    pub const fn has_setter(&self) -> bool {
        !matches!(self.set, Value::Undefined)
    }

    pub const fn enumerable(&self) -> PropertyEnumerable {
        self.enumerable
    }

    pub const fn configurable(&self) -> PropertyConfigurable {
        self.configurable
    }
}

/// Partial accessor descriptor used by define paths; absent fields keep the
/// existing state or fall back to defineProperty defaults for new properties.
#[derive(Debug, Clone)]
pub struct AccessorPropertyUpdate {
    get: Option<Value>,
    set: Option<Value>,
    enumerable: Option<PropertyEnumerable>,
    configurable: Option<PropertyConfigurable>,
}

impl AccessorPropertyUpdate {
    pub const fn new(
        get: Option<Value>,
        set: Option<Value>,
        enumerable: Option<PropertyEnumerable>,
        configurable: Option<PropertyConfigurable>,
    ) -> Self {
        Self {
            get,
            set,
            enumerable,
            configurable,
        }
    }

    pub fn complete_for_new(self) -> AccessorPropertyDescriptor {
        AccessorPropertyDescriptor::new(
            self.get.unwrap_or(Value::Undefined),
            self.set.unwrap_or(Value::Undefined),
            self.enumerable.unwrap_or(PropertyEnumerable::No),
            self.configurable.unwrap_or(PropertyConfigurable::No),
        )
    }

    pub(in crate::runtime) const fn trace_values(&self) -> [Option<&Value>; 2] {
        [self.get.as_ref(), self.set.as_ref()]
    }
}

/// A property definition request that either carries data-descriptor fields
/// or accessor-descriptor fields.
#[derive(Debug, Clone)]
pub enum PropertyUpdate {
    Data(DataPropertyUpdate),
    Accessor(AccessorPropertyUpdate),
}

impl PropertyUpdate {
    pub(in crate::runtime) const fn trace_values(&self) -> [Option<&Value>; 2] {
        match self {
            Self::Data(update) => update.trace_values(),
            Self::Accessor(update) => update.trace_values(),
        }
    }

    fn complete_for_new(self) -> ObjectProperty {
        match self {
            Self::Data(update) => ObjectProperty::from_descriptor(update.complete_for_new()),
            Self::Accessor(update) => {
                ObjectProperty::from_accessor_descriptor(update.complete_for_new())
            }
        }
    }
}

/// Snapshot of one own property as either a data or an accessor descriptor.
#[derive(Debug, Clone)]
pub enum OwnPropertyDescriptor {
    Data(DataPropertyDescriptor),
    Accessor(AccessorPropertyDescriptor),
}

#[derive(Debug, Clone)]
enum ObjectPropertyPayload {
    Data(DataPropertyDescriptor),
    Accessor(AccessorPropertyDescriptor),
}

#[derive(Debug, Clone)]
pub struct ObjectProperty {
    payload: ObjectPropertyPayload,
    version: u64,
}

impl ObjectProperty {
    pub const fn ordinary(value: Value, enumerable: PropertyEnumerable) -> Self {
        Self {
            payload: ObjectPropertyPayload::Data(DataPropertyDescriptor::new(
                value,
                PropertyWritable::Yes,
                enumerable,
                PropertyConfigurable::Yes,
            )),
            version: 0,
        }
    }

    pub(in crate::runtime::object) const fn from_descriptor(
        descriptor: DataPropertyDescriptor,
    ) -> Self {
        Self {
            payload: ObjectPropertyPayload::Data(descriptor),
            version: 0,
        }
    }

    const fn from_accessor_descriptor(descriptor: AccessorPropertyDescriptor) -> Self {
        Self {
            payload: ObjectPropertyPayload::Accessor(descriptor),
            version: 0,
        }
    }

    pub(in crate::runtime) fn from_update(update: PropertyUpdate) -> Self {
        match update {
            PropertyUpdate::Data(update) => Self::from_descriptor(update.complete_for_new()),
            PropertyUpdate::Accessor(update) => {
                Self::from_accessor_descriptor(update.complete_for_new())
            }
        }
    }

    pub fn value(&self) -> Value {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => descriptor.value(),
            ObjectPropertyPayload::Accessor(_) => Value::Undefined,
        }
    }

    /// Borrows the stored data value; `None` for accessor properties, which
    /// have no direct value slot.
    pub const fn data_value_ref(&self) -> Option<&Value> {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => Some(descriptor.value_ref()),
            ObjectPropertyPayload::Accessor(_) => None,
        }
    }

    pub(in crate::runtime) fn visit_strong_edges<Kind: Copy, V: StrongEdgeVisitor<Kind>>(
        &self,
        kind: Kind,
        visitor: &mut V,
    ) -> Result<()> {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => {
                visitor.visit(kind, StrongEdgeReference::Value(descriptor.value_ref()))
            }
            ObjectPropertyPayload::Accessor(descriptor) => {
                visitor.visit(kind, StrongEdgeReference::Value(descriptor.get_ref()))?;
                visitor.visit(kind, StrongEdgeReference::Value(descriptor.set_ref()))
            }
        }
    }

    pub const fn is_accessor(&self) -> bool {
        matches!(self.payload, ObjectPropertyPayload::Accessor(_))
    }

    pub const fn accessor(&self) -> Option<&AccessorPropertyDescriptor> {
        match &self.payload {
            ObjectPropertyPayload::Accessor(descriptor) => Some(descriptor),
            ObjectPropertyPayload::Data(_) => None,
        }
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub const fn is_enumerable(&self) -> bool {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => descriptor.enumerable().is_yes(),
            ObjectPropertyPayload::Accessor(descriptor) => descriptor.enumerable().is_yes(),
        }
    }

    pub const fn is_configurable(&self) -> bool {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => descriptor.configurable().is_yes(),
            ObjectPropertyPayload::Accessor(descriptor) => descriptor.configurable().is_yes(),
        }
    }

    pub const fn is_writable(&self) -> bool {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => descriptor.writable().is_yes(),
            ObjectPropertyPayload::Accessor(_) => false,
        }
    }

    pub const fn is_frozen(&self) -> bool {
        !self.is_configurable() && !self.is_writable()
    }

    pub(in crate::runtime::object) const fn has_default_array_attributes(&self) -> bool {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => {
                descriptor.writable().is_yes()
                    && descriptor.enumerable().is_yes()
                    && descriptor.configurable().is_yes()
            }
            ObjectPropertyPayload::Accessor(_) => false,
        }
    }

    pub fn own_descriptor(&self) -> OwnPropertyDescriptor {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => {
                OwnPropertyDescriptor::Data(descriptor.clone())
            }
            ObjectPropertyPayload::Accessor(descriptor) => {
                OwnPropertyDescriptor::Accessor(descriptor.clone())
            }
        }
    }

    pub(in crate::runtime::object) const fn shape_attributes(&self) -> ShapePropertyAttributes {
        match &self.payload {
            ObjectPropertyPayload::Data(descriptor) => ShapePropertyAttributes::new(
                descriptor.writable().is_yes(),
                descriptor.enumerable().is_yes(),
                descriptor.configurable().is_yes(),
            ),
            ObjectPropertyPayload::Accessor(descriptor) => ShapePropertyAttributes::new(
                descriptor.has_setter(),
                descriptor.enumerable().is_yes(),
                descriptor.configurable().is_yes(),
            ),
        }
    }

    pub fn set_value(&mut self, value: Value) {
        // Accessor slots never store a direct value; setter dispatch happens
        // before this level, so a stray write here must stay a no-op.
        if let ObjectPropertyPayload::Data(descriptor) = &mut self.payload
            && descriptor.writable().is_yes()
        {
            descriptor.value = value;
            self.version = self.version.saturating_add(1);
        }
    }

    pub fn define(&mut self, update: PropertyUpdate) -> Result<()> {
        self.validate_update(&update)?;
        match update {
            PropertyUpdate::Data(update) if update.is_generic() => {
                self.define_generic(update.enumerable, update.configurable);
            }
            PropertyUpdate::Data(update) => self.define_data(update),
            PropertyUpdate::Accessor(update) => self.define_accessor(update),
        }
        Ok(())
    }

    pub(in crate::runtime::object) const fn seal(&mut self) {
        match &mut self.payload {
            ObjectPropertyPayload::Data(descriptor) => {
                descriptor.configurable = PropertyConfigurable::No;
            }
            ObjectPropertyPayload::Accessor(descriptor) => {
                descriptor.configurable = PropertyConfigurable::No;
            }
        }
        self.version = self.version.saturating_add(1);
    }

    pub(in crate::runtime::object) const fn freeze(&mut self) {
        match &mut self.payload {
            ObjectPropertyPayload::Data(descriptor) => {
                descriptor.writable = PropertyWritable::No;
                descriptor.configurable = PropertyConfigurable::No;
            }
            ObjectPropertyPayload::Accessor(descriptor) => {
                descriptor.configurable = PropertyConfigurable::No;
            }
        }
        self.version = self.version.saturating_add(1);
    }

    fn define_data(&mut self, update: DataPropertyUpdate) {
        match &mut self.payload {
            ObjectPropertyPayload::Data(descriptor) => {
                if let Some(value) = update.value {
                    descriptor.value = value;
                    self.version = self.version.saturating_add(1);
                }
                if let Some(writable) = update.writable {
                    descriptor.writable = writable;
                }
                if let Some(enumerable) = update.enumerable {
                    descriptor.enumerable = enumerable;
                }
                if let Some(configurable) = update.configurable {
                    descriptor.configurable = configurable;
                }
            }
            ObjectPropertyPayload::Accessor(existing) => {
                let enumerable = update.enumerable.unwrap_or(existing.enumerable);
                let configurable = update.configurable.unwrap_or(existing.configurable);
                self.payload = ObjectPropertyPayload::Data(DataPropertyDescriptor::new(
                    update.value.unwrap_or(Value::Undefined),
                    update.writable.unwrap_or(PropertyWritable::No),
                    enumerable,
                    configurable,
                ));
                self.version = self.version.saturating_add(1);
            }
        }
    }

    const fn define_generic(
        &mut self,
        enumerable: Option<PropertyEnumerable>,
        configurable: Option<PropertyConfigurable>,
    ) {
        match &mut self.payload {
            ObjectPropertyPayload::Data(descriptor) => {
                if let Some(enumerable) = enumerable {
                    descriptor.enumerable = enumerable;
                }
                if let Some(configurable) = configurable {
                    descriptor.configurable = configurable;
                }
            }
            ObjectPropertyPayload::Accessor(descriptor) => {
                if let Some(enumerable) = enumerable {
                    descriptor.enumerable = enumerable;
                }
                if let Some(configurable) = configurable {
                    descriptor.configurable = configurable;
                }
            }
        }
        self.version = self.version.saturating_add(1);
    }

    fn define_accessor(&mut self, update: AccessorPropertyUpdate) {
        match &mut self.payload {
            ObjectPropertyPayload::Accessor(descriptor) => {
                if let Some(get) = update.get {
                    descriptor.get = get;
                }
                if let Some(set) = update.set {
                    descriptor.set = set;
                }
                if let Some(enumerable) = update.enumerable {
                    descriptor.enumerable = enumerable;
                }
                if let Some(configurable) = update.configurable {
                    descriptor.configurable = configurable;
                }
            }
            ObjectPropertyPayload::Data(existing) => {
                let enumerable = update.enumerable.unwrap_or(existing.enumerable);
                let configurable = update.configurable.unwrap_or(existing.configurable);
                self.payload = ObjectPropertyPayload::Accessor(AccessorPropertyDescriptor::new(
                    update.get.unwrap_or(Value::Undefined),
                    update.set.unwrap_or(Value::Undefined),
                    enumerable,
                    configurable,
                ));
            }
        }
        self.version = self.version.saturating_add(1);
    }

    pub const fn set_enumerable(&mut self, enumerable: PropertyEnumerable) {
        match &mut self.payload {
            ObjectPropertyPayload::Data(descriptor) => descriptor.enumerable = enumerable,
            ObjectPropertyPayload::Accessor(descriptor) => descriptor.enumerable = enumerable,
        }
    }
}

impl ObjectHeap {
    pub fn own_property_descriptor(
        &self,
        id: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        self.object(id)
            .and_then(|object| object.own_property_descriptor(property, &self.shapes))
    }

    pub fn define_property(
        &mut self,
        id: ObjectId,
        property: PropertyKey,
        property_name: &str,
        update: PropertyUpdate,
        max_properties: usize,
    ) -> Result<()> {
        let before = self.object(id)?.structure_snapshot();
        let (object, shapes) = self.object_mut_with_shapes(id)?;
        object.define_property(property, property_name, update, shapes, max_properties)?;
        self.bump_prototype_lookup_version()?;
        self.bump_if_structure_changed(id, before)
    }

    pub fn has_own(&self, id: ObjectId, property: PropertyLookup<'_>) -> Result<bool> {
        self.object(id)
            .and_then(|object| object.has_own(property, &self.shapes))
    }
}

impl Object {
    fn own_property_descriptor(
        &self,
        property: PropertyLookup<'_>,
        shapes: &ShapeTable,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        if let Some(length) = self
            .array_length
            .filter(|_| property.name() == ARRAY_LENGTH_PROPERTY)
        {
            return Ok(Some(OwnPropertyDescriptor::Data(
                DataPropertyDescriptor::new(
                    length.value(),
                    self.array_length_writable,
                    PropertyEnumerable::No,
                    PropertyConfigurable::No,
                ),
            )));
        }
        if self.array_length.is_some()
            && let Some(index) = ArrayIndex::parse(property.name())
            && let Some(descriptor) = self.array_element_descriptor(index)
        {
            return Ok(Some(descriptor));
        }
        let Some(key) = property.key() else {
            return Ok(None);
        };
        self.named_property(shapes, key)
            .map(|property| property.map(ObjectProperty::own_descriptor))
    }

    pub(in crate::runtime::object) fn define_property(
        &mut self,
        property: PropertyKey,
        property_name: &str,
        update: PropertyUpdate,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        let index = ArrayIndex::parse(property_name);
        if self.has_virtual_string_property_name(property_name)? {
            return Ok(());
        }
        if self.array_length.is_some()
            && let Some(index) = index
        {
            return self.define_array_property(index, property, update, shapes, max_properties);
        }
        self.define_named_property(property, update, shapes, max_properties)?;
        if let Some(index) = index {
            self.array_storage.insert_sparse_key(index, property);
        }
        Ok(())
    }

    fn define_named_property(
        &mut self,
        property: PropertyKey,
        update: PropertyUpdate,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        let property_count = self.property_count();
        let enumerable_update = if self.contains_named_property(shapes, property)? {
            let (was_enumerable, is_enumerable, attributes) = {
                let existing = self.named_property_mut(shapes, property)?;
                let was_enumerable = existing.is_enumerable();
                existing.define(update)?;
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
                return Err(crate::error::Error::type_error(
                    "cannot define property on non-extensible object",
                ));
            }
            if property_count >= max_properties {
                return Err(crate::error::Error::limit(format!(
                    "object property count exceeded {max_properties}"
                )));
            }
            let named_property = update.complete_for_new();
            let enumerable_update = named_property.is_enumerable().then_some((false, true));
            self.push_named_property(shapes, property, named_property)?;
            enumerable_update
        };
        if let Some((was_enumerable, is_enumerable)) = enumerable_update {
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
        }
        Ok(())
    }

    fn define_array_property(
        &mut self,
        index: ArrayIndex,
        property: PropertyKey,
        update: PropertyUpdate,
        shapes: &mut ShapeTable,
        max_properties: usize,
    ) -> Result<()> {
        if self
            .array_length
            .is_some_and(|length| !length.contains(index))
            && !self.array_length_writable.is_yes()
        {
            return Err(crate::error::Error::type_error(
                "cannot define an array index beyond non-writable length",
            ));
        }
        if index.dense_position(max_properties)?.is_none() {
            if !self.extensibility.is_extensible() {
                return Err(crate::error::Error::type_error(
                    "cannot define property on non-extensible object",
                ));
            }
            self.define_named_property(property, update, shapes, max_properties)?;
            self.array_storage.insert_sparse_key(index, property);
            return self.extend_array_length(index);
        }
        let has_existing = self.array_storage.dense_property(index).is_some();
        if !has_existing && !self.extensibility.is_extensible() {
            return Err(crate::error::Error::type_error(
                "cannot define property on non-extensible object",
            ));
        }
        if !has_existing && self.property_count() >= max_properties {
            return Err(crate::error::Error::limit(format!(
                "object property count exceeded {max_properties}"
            )));
        }
        if let Some(existing) = self.array_storage.dense_property_mut(index)? {
            let was_enumerable = existing.is_enumerable();
            existing.define(update)?;
            let is_enumerable = existing.is_enumerable();
            self.update_enumerable_property_count(was_enumerable, is_enumerable);
        } else {
            let property = ObjectProperty::from_update(update);
            let is_enumerable = property.is_enumerable();
            let reservation = self.reserve_property_growth()?;
            let previous = self.array_storage.insert_dense_property(index, property)?;
            if previous.is_some() {
                return Err(crate::error::Error::runtime(
                    "array index storage replaced existing slot",
                ));
            }
            if let Some(reservation) = reservation {
                reservation.commit()?;
            }
            if is_enumerable {
                self.enumerable_property_count = self.enumerable_property_count.saturating_add(1);
            }
        }
        self.extend_array_length(index)
    }

    fn array_element_descriptor(&self, index: ArrayIndex) -> Option<OwnPropertyDescriptor> {
        self.array_storage
            .dense_property(index)
            .map(ObjectProperty::own_descriptor)
    }
}
