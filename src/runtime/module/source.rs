#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        binding::scope::BindingCell,
        native::OBJECT_CONSTRUCTOR_PROPERTY,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, ObjectPropertyInit, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable,
        },
    },
    syntax::DeclKind,
    value::{ErrorName, Value},
};

use super::PendingModule;

const ABSTRACT_MODULE_SOURCE_NAME: &str = "AbstractModuleSource";
const PROTOTYPE_PROPERTY: &str = "prototype";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";

impl Context {
    pub(super) fn initialize_module_source_objects(
        &mut self,
        graph: &mut [PendingModule],
    ) -> Result<()> {
        for pending in graph {
            let Some(class_name) = pending.module_source_class_name.clone() else {
                continue;
            };
            let (source, binding) = self.create_module_source_binding(&class_name)?;
            pending.module_source = Some(source);
            pending.module_source_binding = Some(binding);
        }
        Ok(())
    }

    pub(super) fn required_module_source_binding(
        graph: &[PendingModule],
        module_index: usize,
    ) -> Result<BindingCell> {
        let module = graph
            .get(module_index)
            .ok_or_else(|| Error::runtime("module source owner is missing"))?;
        module
            .module_source_binding
            .clone()
            .ok_or_else(|| Self::module_source_unavailable(&module.name))
    }

    pub(super) fn module_source_unavailable(module_name: &str) -> Error {
        Error::exception(
            ErrorName::SyntaxError,
            format!("source phase import is unavailable for source text module '{module_name}'"),
        )
    }

    pub(crate) fn abstract_module_source_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.realm.abstract_module_source_constructor {
            return Ok(Value::HostFunction(id));
        }
        self.object_constructor_value()?;
        let constructor =
            self.create_internal_host_function(ABSTRACT_MODULE_SOURCE_NAME.to_owned(), |_call| {
                Err(Error::type_error(
                    "AbstractModuleSource cannot be constructed",
                ))
            })?;
        let Value::HostFunction(constructor_id) = constructor else {
            return Err(Error::runtime(
                "AbstractModuleSource constructor is not a host function",
            ));
        };
        let constructor_key = self.object_constructor_property_key()?;
        let prototype = self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor.clone(),
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let prototype_key = self.intern_property_key(PROTOTYPE_PROPERTY)?;
        self.define_host_function_property_key(
            constructor_id,
            PROTOTYPE_PROPERTY,
            prototype_key,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(Value::Object(prototype)),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
        )?;
        self.install_abstract_module_source_to_string_tag(prototype)?;
        self.storage_ledger
            .grow_count(VmStorageKind::Association, 2)?;
        self.realm.abstract_module_source_constructor = Some(constructor_id);
        self.realm.abstract_module_source_prototype = Some(prototype);
        Ok(constructor)
    }

    fn install_abstract_module_source_to_string_tag(
        &mut self,
        prototype: crate::value::ObjectId,
    ) -> Result<()> {
        let getter = self
            .create_internal_host_function("get [Symbol.toStringTag]".to_owned(), |_call| {
                Ok(Value::Undefined)
            })?;
        let symbol = self.symbol_constructor_value()?;
        let Value::Symbol(tag) = self.get_named(&symbol, "toStringTag")? else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(tag.id()),
            TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(super) fn create_module_source_binding(
        &mut self,
        class_name: &str,
    ) -> Result<(Value, BindingCell)> {
        self.check_string_len(class_name)?;
        self.abstract_module_source_constructor_value()?;
        let prototype = self
            .realm
            .abstract_module_source_prototype
            .ok_or_else(|| Error::runtime("AbstractModuleSource prototype is not initialized"))?;
        let source = self
            .objects
            .create_with_exact_prototype(Some(prototype), self.limits.max_objects)?;
        let Value::Object(source_id) = source else {
            return Err(Error::runtime("module source object allocation failed"));
        };
        let symbol = self.symbol_constructor_value()?;
        let Value::Symbol(tag) = self.get_named(&symbol, "toStringTag")? else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let class_name = self.heap_string_value(class_name)?;
        self.objects.define_property(
            source_id,
            PropertyKey::symbol(tag.id()),
            TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(class_name),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )?;
        let binding = BindingCell::new(source.clone(), false, DeclKind::Const);
        Ok((source, binding))
    }
}
