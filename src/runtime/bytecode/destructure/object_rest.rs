#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::Result,
    runtime::{
        Context,
        object::{
            OBJECT_CONSTRUCTOR_PROPERTY, ObjectPropertyInit, OwnPropertyDescriptor,
            PropertyEnumerable, PropertyKey,
        },
    },
    value::Value,
};

impl Context {
    pub(super) fn destructure_rest_object(
        &mut self,
        source: &Value,
        consumed: &[PropertyKey],
    ) -> Result<Value> {
        let keys = self.semantic_own_property_keys(source)?;
        let mut entries = Vec::new();
        for key_value in keys {
            let mut property = self.dynamic_property_key(&key_value)?;
            let property_key = self.intern_dynamic_property_key(&mut property)?;
            if consumed.contains(&property_key) {
                continue;
            }
            let Some(descriptor) = self.semantic_own_property_descriptor(source, &property)? else {
                continue;
            };
            let enumerable = match descriptor {
                OwnPropertyDescriptor::Data(descriptor) => descriptor.enumerable(),
                OwnPropertyDescriptor::Accessor(descriptor) => descriptor.enumerable(),
            };
            if !enumerable.is_yes() {
                continue;
            }
            let value = self.get(source, property.lookup())?;
            entries.push((property_key, property.name().to_owned(), value));
        }
        let inits = entries
            .iter()
            .map(|(key, name, value)| {
                ObjectPropertyInit::new_data(
                    *key,
                    name.as_str(),
                    value.clone(),
                    PropertyEnumerable::Yes,
                )
            })
            .collect::<Vec<_>>();
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.create(
            inits,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }
}
