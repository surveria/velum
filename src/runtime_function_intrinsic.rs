use crate::{
    runtime_object::{
        DataPropertyDescriptor, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
        PropertyWritable,
    },
    value::Value,
};

#[derive(Debug, Clone)]
pub(super) struct FunctionIntrinsicProperty {
    descriptor: Option<DataPropertyDescriptor>,
    deleted: bool,
}

impl FunctionIntrinsicProperty {
    pub(super) const fn new() -> Self {
        Self {
            descriptor: None,
            deleted: false,
        }
    }

    pub(super) const fn has(&self) -> bool {
        !self.deleted
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

    pub(super) fn set_value(&mut self, default: DataPropertyDescriptor, value: Value) -> bool {
        let Some(descriptor) = self.descriptor(default) else {
            return false;
        };
        if descriptor.writable().is_yes() {
            self.descriptor = Some(DataPropertyDescriptor::new(
                value,
                descriptor.writable(),
                descriptor.enumerable(),
                descriptor.configurable(),
            ));
        }
        true
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
        Some(true)
    }
}

#[derive(Debug, Clone)]
pub(super) struct FunctionProperty {
    descriptor: DataPropertyDescriptor,
}

impl FunctionProperty {
    pub(super) const fn new(value: Value, enumerable: PropertyEnumerable) -> Self {
        Self {
            descriptor: DataPropertyDescriptor::new(
                value,
                PropertyWritable::Yes,
                enumerable,
                PropertyConfigurable::Yes,
            ),
        }
    }

    pub(super) fn from_update(update: DataPropertyUpdate) -> Self {
        Self {
            descriptor: update.complete_for_new(),
        }
    }

    pub(super) fn value(&self) -> Value {
        self.descriptor.value()
    }

    pub(super) const fn is_configurable(&self) -> bool {
        self.descriptor.configurable().is_yes()
    }

    pub(super) const fn is_enumerable(&self) -> bool {
        self.descriptor.enumerable().is_yes()
    }

    pub(super) fn descriptor(&self) -> DataPropertyDescriptor {
        self.descriptor.clone()
    }

    pub(super) fn set_value(&mut self, value: Value) {
        if self.descriptor.writable().is_yes() {
            self.descriptor = DataPropertyDescriptor::new(
                value,
                self.descriptor.writable(),
                self.descriptor.enumerable(),
                self.descriptor.configurable(),
            );
        }
    }

    pub(super) fn define(&mut self, update: &DataPropertyUpdate) {
        let value = update.value().unwrap_or_else(|| self.descriptor.value());
        let writable = update
            .writable()
            .unwrap_or_else(|| self.descriptor.writable());
        let enumerable = update
            .enumerable()
            .unwrap_or_else(|| self.descriptor.enumerable());
        let configurable = update
            .configurable()
            .unwrap_or_else(|| self.descriptor.configurable());
        self.descriptor = DataPropertyDescriptor::new(value, writable, enumerable, configurable);
    }

    pub(super) fn set_enumerable(&mut self, enumerable: PropertyEnumerable) {
        self.descriptor = DataPropertyDescriptor::new(
            self.descriptor.value(),
            self.descriptor.writable(),
            enumerable,
            self.descriptor.configurable(),
        );
    }
}
