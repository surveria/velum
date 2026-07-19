#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::Result,
    runtime::{Context, object::PropertyKey, property::DynamicPropertyKey},
    syntax::AccessorKind,
    value::Value,
};

const GETTER_NAME_PREFIX: &str = "get";
const SETTER_NAME_PREFIX: &str = "set";

impl Context {
    pub(in crate::runtime) fn set_function_name(
        &mut self,
        value: &Value,
        name: &str,
        prefix: Option<AccessorKind>,
    ) -> Result<()> {
        let Value::Function(id) = value else {
            return Ok(());
        };
        let name = match prefix {
            Some(AccessorKind::Getter) => format!("{GETTER_NAME_PREFIX} {name}"),
            Some(AccessorKind::Setter) => format!("{SETTER_NAME_PREFIX} {name}"),
            None => name.to_owned(),
        };
        self.check_string_len(&name)?;
        self.set_generated_function_name(*id, &name)
    }

    pub(in crate::runtime) fn set_function_name_from_property(
        &mut self,
        value: &Value,
        property: &DynamicPropertyKey,
        prefix: Option<AccessorKind>,
    ) -> Result<()> {
        let name = self.function_name_from_property(property)?;
        self.set_function_name(value, &name, prefix)
    }

    pub(in crate::runtime) fn function_name_from_property(
        &self,
        property: &DynamicPropertyKey,
    ) -> Result<String> {
        let Some(symbol) = property.key().and_then(PropertyKey::symbol_id) else {
            return Ok(property.name().to_owned());
        };
        Ok(self
            .symbols
            .get(symbol)?
            .description()
            .map_or_else(String::new, |description| format!("[{description}]")))
    }
}
