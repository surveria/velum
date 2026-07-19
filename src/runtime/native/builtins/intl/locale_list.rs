#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{Context, object::IntlValue},
    value::Value,
};

impl Context {
    pub(in crate::runtime::native) fn intl_locale_list(
        &mut self,
        value: &Value,
    ) -> Result<Vec<String>> {
        if matches!(value, Value::Undefined) {
            return Ok(Vec::new());
        }
        if value.string_text().is_some() || self.is_intl_locale_value(value)? {
            let tag = self.locale_source_tag(value)?;
            return Ok(vec![super::locale::canonicalize_locale_tag(&tag)?]);
        }
        let object = self.object_to_object(value)?;
        let length_value = self.get_named(&object, "length")?;
        let length = Self::length_to_usize(
            self.to_length(&length_value)?,
            "Intl locale list length exceeded supported range",
        )?;
        let mut locales = Vec::new();
        for index in 0..length {
            self.step()?;
            let name = index.to_string();
            let lookup = self.property_lookup(&name);
            if !self.has_property_value_with_lookup(&object, lookup)? {
                continue;
            }
            let item = self.get_named(&object, &name)?;
            if item.string_text().is_none() && !is_object_value(&item) {
                return Err(Error::type_error("Intl locale entry is invalid"));
            }
            let tag = self.locale_source_tag(&item)?;
            let locale = super::locale::canonicalize_locale_tag(&tag)?;
            if !locales.contains(&locale) {
                locales.push(locale);
            }
        }
        Ok(locales)
    }

    fn is_intl_locale_value(&self, value: &Value) -> Result<bool> {
        let Value::Object(id) = value else {
            return Ok(false);
        };
        Ok(matches!(
            self.objects.intl_value(*id)?,
            Some(IntlValue::Locale(_))
        ))
    }
}

const fn is_object_value(value: &Value) -> bool {
    matches!(
        value,
        Value::Object(_) | Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_)
    )
}
