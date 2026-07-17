use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        control::runtime_exception_value,
        object::{OwnPropertyDescriptor, PropertyUpdate},
    },
    value::Value,
};

use super::RetainedValue;

#[derive(Clone, Copy, Debug)]
pub enum EmbeddingObjectPrototype<'value> {
    Default,
    Null,
    Explicit(&'value RetainedValue),
}

impl Context {
    pub(crate) fn resolve_embedding_object_prototype(
        &mut self,
        prototype: EmbeddingObjectPrototype<'_>,
    ) -> Result<Option<Value>> {
        match prototype {
            EmbeddingObjectPrototype::Null => Ok(None),
            EmbeddingObjectPrototype::Explicit(prototype) => {
                let value = self.resolve_retained_value(prototype)?;
                if matches!(value, Value::Null) {
                    return Ok(None);
                }
                if self.semantic_object_ref(&value)?.is_some() {
                    return Ok(Some(value));
                }
                Err(Error::runtime(
                    "embedding object prototype must be an object or null",
                ))
            }
            EmbeddingObjectPrototype::Default => {
                let constructor_key = self.object_constructor_property_key()?;
                let prototype = self.objects.object_prototype_id(
                    constructor_key,
                    self.limits.max_objects,
                    self.limits.max_object_properties,
                )?;
                Ok(Some(Value::Object(prototype)))
            }
        }
    }

    pub(crate) fn create_embedding_object(
        &mut self,
        prototype: EmbeddingObjectPrototype<'_>,
    ) -> Result<RetainedValue> {
        let prototype = self.resolve_embedding_object_prototype(prototype)?;
        self.objects.reserve_created_object_rollback()?;
        let value = self
            .objects
            .create_with_semantic_prototype(prototype, self.limits.max_objects)?;
        let Value::Object(id) = value else {
            return Err(Error::runtime(
                "ordinary object creation returned a non-object value",
            ));
        };
        match self.retain_embedder_value(Value::Object(id)) {
            Ok(value) => Ok(value),
            Err(error) => {
                self.objects.discard_created_empty_object(id)?;
                Err(error)
            }
        }
    }

    pub(crate) fn embedding_callable_status(&self, value: &Value) -> Result<bool> {
        self.semantic_is_callable(value)
    }

    pub(crate) fn embedding_constructor_status(&self, value: &Value) -> Result<bool> {
        self.semantic_is_constructor(value)
    }

    pub(crate) fn embedding_call(
        &mut self,
        callable: &Value,
        args: &[Value],
        receiver: Value,
    ) -> Result<Value> {
        let operation = self.semantic_call(callable, args, receiver);
        let completion = self.embedding_result(operation)?;
        let result = completion.into_native_value_result();
        self.embedding_result(result)
    }

    pub(crate) fn embedding_call_method(
        &mut self,
        target: &Value,
        property: &Value,
        args: &[Value],
    ) -> Result<Value> {
        let operation = (|| {
            let property = self.dynamic_property_key(property)?;
            let method = self.get(target, property.lookup())?;
            self.semantic_call(&method, args, target.clone())
        })();
        let completion = self.embedding_result(operation)?;
        let result = completion.into_native_value_result();
        self.embedding_result(result)
    }

    pub(crate) fn embedding_construct(
        &mut self,
        constructor: &Value,
        args: &[Value],
    ) -> Result<Value> {
        let operation = self.semantic_construct(constructor, args, constructor.clone());
        self.embedding_result(operation)
    }

    pub(crate) fn embedding_get_property(
        &mut self,
        target: &Value,
        property: &Value,
    ) -> Result<Value> {
        let operation = (|| {
            let property = self.dynamic_property_key(property)?;
            self.get(target, property.lookup())
        })();
        self.embedding_result(operation)
    }

    pub(crate) fn embedding_set_property(
        &mut self,
        target: &Value,
        property: &Value,
        value: Value,
        throw_on_failure: bool,
    ) -> Result<bool> {
        let operation = (|| {
            let mut property = self.dynamic_property_key(property)?;
            let updated = self
                .semantic_reflect_property_write(target, &mut property, value, target)?
                .ok_or_else(|| Error::type_error("Set target is not an object"))?;
            if !updated && throw_on_failure {
                return Err(Error::type_error(format!(
                    "Cannot assign to property '{}'",
                    property.name()
                )));
            }
            Ok(updated)
        })();
        self.embedding_result(operation)
    }

    pub(crate) fn embedding_define_property(
        &mut self,
        target: &Value,
        property: &Value,
        update: PropertyUpdate,
        throw_on_failure: bool,
    ) -> Result<bool> {
        let operation = (|| {
            self.validate_embedding_property_update(&update)?;
            let descriptor = self.create_property_update_object(&update)?;
            let mut property = self.dynamic_property_key(property)?;
            let defined = self.semantic_define_own_property_update_with_descriptor(
                target,
                &mut property,
                update,
                &descriptor,
            )?;
            if !defined && throw_on_failure {
                return Err(Error::type_error(format!(
                    "Cannot define property '{}'",
                    property.name()
                )));
            }
            Ok(defined)
        })();
        self.embedding_result(operation)
    }

    pub(crate) fn embedding_delete_property(
        &mut self,
        target: &Value,
        property: &Value,
        throw_on_failure: bool,
    ) -> Result<bool> {
        let operation = (|| {
            let property = self.dynamic_property_key(property)?;
            let deleted = self.delete_property_value_with_lookup(target, property.lookup())?;
            if !deleted && throw_on_failure {
                return Err(Error::type_error(format!(
                    "Cannot delete property '{}'",
                    property.name()
                )));
            }
            Ok(deleted)
        })();
        self.embedding_result(operation)
    }

    pub(crate) fn embedding_own_property_descriptor(
        &mut self,
        target: &Value,
        property: &Value,
    ) -> Result<Option<OwnPropertyDescriptor>> {
        let operation = (|| {
            let property = self.dynamic_property_key(property)?;
            self.semantic_own_property_descriptor(target, &property)
        })();
        self.embedding_result(operation)
    }

    fn validate_embedding_property_update(&self, update: &PropertyUpdate) -> Result<()> {
        let PropertyUpdate::Accessor(update) = update else {
            return Ok(());
        };
        for (label, value) in [("getter", &update.get), ("setter", &update.set)] {
            let Some(value) = value else {
                continue;
            };
            if !matches!(value, Value::Undefined) && !self.semantic_is_callable(value)? {
                return Err(Error::type_error(format!(
                    "property descriptor {label} must be callable or undefined"
                )));
            }
        }
        Ok(())
    }

    fn embedding_result<T>(&mut self, result: Result<T>) -> Result<T> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                let diagnostic_span = error.source_span();
                let Some(value) = runtime_exception_value(self, &error)? else {
                    return Err(error);
                };
                let metadata = if let Value::Object(id) = &value {
                    self.objects.error_metadata(*id)?.cloned()
                } else {
                    None
                };
                Err(Error::javascript_with_metadata(
                    self.identity().clone(),
                    value,
                    metadata,
                    diagnostic_span,
                ))
            }
        }
    }
}
