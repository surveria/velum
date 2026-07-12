use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable,
        },
    },
    value::{ErrorName, ObjectId, Value},
};

mod duration;
mod install;

use crate::runtime::native::{TEMPORAL_NAME, TemporalFunctionKind};

const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";

impl Context {
    pub(in crate::runtime::native) fn temporal_namespace_value(&mut self) -> Result<Value> {
        if let Some(binding) = self.get_binding(TEMPORAL_NAME) {
            return binding.value(TEMPORAL_NAME);
        }
        self.object_constructor_value()?;
        let constructor_key = self.object_constructor_property_key()?;
        let namespace = self.objects.create_with_prototype_id(
            None,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )?;
        let duration = self.temporal_duration_constructor_value()?;
        self.define_non_enumerable_object_property(namespace, "Duration", duration)?;
        self.define_temporal_to_string_tag(namespace, TEMPORAL_NAME)?;
        let value = Value::Object(namespace);
        self.insert_global_builtin(TEMPORAL_NAME, value.clone())?;
        Ok(value)
    }

    fn define_temporal_to_string_tag(&mut self, object: ObjectId, tag: &str) -> Result<()> {
        let constructor = self.symbol_constructor_value()?;
        let symbol = self.get_named(&constructor, SYMBOL_TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(tag)?;
        self.objects.define_property(
            object,
            PropertyKey::symbol(symbol.id()),
            SYMBOL_TO_STRING_TAG_PROPERTY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}

fn temporal_error(error: temporal_rs::TemporalError) -> Error {
    let message = error.to_string();
    match error.kind() {
        temporal_rs::error::ErrorKind::Type => Error::exception(ErrorName::TypeError, message),
        temporal_rs::error::ErrorKind::Range | temporal_rs::error::ErrorKind::Syntax => {
            Error::exception(ErrorName::RangeError, message)
        }
        temporal_rs::error::ErrorKind::Generic => Error::exception(ErrorName::Base, message),
        temporal_rs::error::ErrorKind::Assert => Error::runtime(message),
    }
}

const fn temporal_kind(kind: TemporalFunctionKind) -> crate::runtime::native::NativeFunctionKind {
    crate::runtime::native::NativeFunctionKind::Temporal(kind)
}
