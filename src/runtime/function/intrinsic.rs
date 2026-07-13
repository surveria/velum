use crate::{
    error::Result,
    runtime::object::{
        DataPropertyDescriptor, DataPropertyUpdate, ObjectProperty, OwnPropertyDescriptor,
        PropertyEnumerable, PropertyUpdate,
    },
    runtime::trace::StrongEdgeVisitor,
    value::Value,
};

#[derive(Debug, Clone)]
pub(super) struct FunctionIntrinsicProperty {
    descriptor: Option<DataPropertyDescriptor>,
    deleted: bool,
    replaced: bool,
}

impl FunctionIntrinsicProperty {
    pub(super) const fn new() -> Self {
        Self {
            descriptor: None,
            deleted: false,
            replaced: false,
        }
    }

    pub(super) const fn has(&self) -> bool {
        !self.deleted
    }

    pub(super) const fn stored_value(&self) -> Option<&Value> {
        match &self.descriptor {
            Some(descriptor) => Some(descriptor.value_ref()),
            None => None,
        }
    }

    pub(super) fn descriptor(
        &self,
        default: DataPropertyDescriptor,
    ) -> Option<DataPropertyDescriptor> {
        if self.deleted {
            return None;
        }
        Some(self.descriptor.clone().unwrap_or(default))
    }

    pub(super) fn value(&self, default: DataPropertyDescriptor) -> Option<Value> {
        self.descriptor(default)
            .map(|descriptor| descriptor.value())
    }

    pub(super) fn define(
        &mut self,
        default: DataPropertyDescriptor,
        update: &DataPropertyUpdate,
    ) -> bool {
        let Some(descriptor) = self.descriptor(default) else {
            return false;
        };
        let value = update.value().unwrap_or_else(|| descriptor.value());
        let writable = update.writable().unwrap_or_else(|| descriptor.writable());
        let enumerable = update
            .enumerable()
            .unwrap_or_else(|| descriptor.enumerable());
        let configurable = update
            .configurable()
            .unwrap_or_else(|| descriptor.configurable());
        self.descriptor = Some(DataPropertyDescriptor::new(
            value,
            writable,
            enumerable,
            configurable,
        ));
        true
    }

    pub(super) fn delete(&mut self, default: DataPropertyDescriptor) -> Option<bool> {
        let descriptor = self.descriptor(default)?;
        if !descriptor.configurable().is_yes() {
            return Some(false);
        }
        self.descriptor = None;
        self.deleted = true;
        self.replaced = false;
        Some(true)
    }

    pub(super) fn replace_with_custom(&mut self, default: DataPropertyDescriptor) -> Option<bool> {
        let descriptor = self.descriptor(default)?;
        if !descriptor.configurable().is_yes() {
            return Some(false);
        }
        self.descriptor = None;
        self.deleted = true;
        self.replaced = true;
        Some(true)
    }

    pub(super) const fn was_replaced(&self) -> bool {
        self.replaced
    }

    pub(super) const fn clear_replaced(&mut self) {
        self.replaced = false;
    }
}

#[derive(Debug, Clone)]
pub(super) struct FunctionProperty {
    property: ObjectProperty,
}

impl FunctionProperty {
    pub(super) const fn new(value: Value, enumerable: PropertyEnumerable) -> Self {
        Self {
            property: ObjectProperty::ordinary(value, enumerable),
        }
    }

    pub(super) fn from_update(update: PropertyUpdate) -> Self {
        Self {
            property: ObjectProperty::from_update(update),
        }
    }

    pub(super) fn visit_strong_edges<Kind: Copy, V: StrongEdgeVisitor<Kind>>(
        &self,
        kind: Kind,
        visitor: &mut V,
    ) -> Result<()> {
        self.property.visit_strong_edges(kind, visitor)
    }

    pub(super) const fn is_configurable(&self) -> bool {
        self.property.is_configurable()
    }

    pub(super) const fn is_frozen(&self) -> bool {
        self.property.is_frozen()
    }

    pub(super) fn descriptor(&self) -> OwnPropertyDescriptor {
        self.property.own_descriptor()
    }

    pub(super) fn set_value(&mut self, value: Value) {
        self.property.set_value(value);
    }

    pub(super) fn define(&mut self, update: PropertyUpdate) -> Result<()> {
        self.property.define(update)
    }

    pub(super) const fn seal(&mut self) {
        self.property.seal();
    }

    pub(super) const fn freeze(&mut self) {
        self.property.freeze();
    }

    pub(super) const fn set_enumerable(&mut self, enumerable: PropertyEnumerable) {
        self.property.set_enumerable(enumerable);
    }
}
