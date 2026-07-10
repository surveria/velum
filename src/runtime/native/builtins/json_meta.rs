use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        object::{
            DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable,
        },
    },
    value::{ObjectId, Value},
};

use super::JSON_NAME;

const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";

impl Context {
    pub(in crate::runtime::native) fn define_json_to_string_tag(
        &mut self,
        object: ObjectId,
    ) -> Result<()> {
        let value = self.heap_string_value(JSON_NAME)?;
        let key = self.json_well_known_symbol_property_key(SYMBOL_TO_STRING_TAG_PROPERTY)?;
        self.objects.define_property(
            object,
            key,
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

    pub(in crate::runtime::native) fn json_parse_text(
        &mut self,
        value: Option<&Value>,
    ) -> Result<String> {
        let Some(value) = value else {
            return self.to_string(&Value::Undefined);
        };
        self.to_string(value)
    }

    fn json_well_known_symbol_property_key(&mut self, property: &str) -> Result<PropertyKey> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, property)?;
        let Value::Symbol(symbol) = value else {
            return Err(Error::runtime("well-known Symbol property is not a symbol"));
        };
        Ok(PropertyKey::symbol(symbol.id()))
    }
}
