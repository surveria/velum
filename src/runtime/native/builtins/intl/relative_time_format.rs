#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{
            DataPropertyUpdate, IntlValue, PropertyConfigurable, PropertyEnumerable,
            PropertyWritable, RelativeTimeFormatValue,
        },
    },
    value::{ErrorName, NativeFunctionId, Value},
};

use super::{
    number_options::{is_unicode_type, resolved_numbering_system},
    relative_time_patterns::relative_time_parts,
};

const RELATIVE_TIME_FORMAT_TAG: &str = "Intl.RelativeTimeFormat";
const SUPPORTED_LOCALES_OF: &str = "supportedLocalesOf";
const DEFAULT_LOCALE: &str = "en-US";
const RELATIVE_TIME_UNITS: &[&str] = &[
    "second", "minute", "hour", "day", "week", "month", "quarter", "year",
];

impl Context {
    pub(in crate::runtime) fn intl_relative_time_format_constructor_value(
        &mut self,
    ) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::RelativeTimeFormatConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let constructor = self.intl_constructor_value(
            constructor_kind,
            RELATIVE_TIME_FORMAT_TAG,
            &[
                ("format", IntlFunctionKind::RelativeTimeFormatFormat),
                (
                    "formatToParts",
                    IntlFunctionKind::RelativeTimeFormatFormatToParts,
                ),
                (
                    "resolvedOptions",
                    IntlFunctionKind::RelativeTimeFormatResolvedOptions,
                ),
            ],
        )?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime(
                "Intl.RelativeTimeFormat constructor is not native",
            ));
        };
        self.install_relative_time_static_method(constructor_id)?;
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(super) fn construct_intl_relative_time_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let options_source = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options_source, Value::Null) {
            return Err(Error::type_error(
                "Intl.RelativeTimeFormat options cannot be null",
            ));
        }
        let options = if matches!(options_source, Value::Undefined) {
            Value::Undefined
        } else {
            self.object_to_object(&options_source)?
        };
        let _matcher = self.relative_time_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let numbering_system = self.relative_time_optional_string(&options, "numberingSystem")?;
        if numbering_system
            .as_deref()
            .is_some_and(|value| !is_unicode_type(value))
        {
            return Err(relative_time_range_error(
                "numberingSystem has an invalid value",
            ));
        }
        let style = self.relative_time_string_option(
            &options,
            "style",
            &["long", "short", "narrow"],
            "long",
        )?;
        let numeric =
            self.relative_time_string_option(&options, "numeric", &["always", "auto"], "always")?;
        let locale = locales
            .into_iter()
            .find(|locale| relative_time_locale_is_supported(locale))
            .unwrap_or_else(|| DEFAULT_LOCALE.to_owned());
        let (locale, numbering_system) = resolved_numbering_system(&locale, numbering_system);
        let prototype =
            self.intl_constructor_prototype(IntlFunctionKind::RelativeTimeFormatConstructor)?;
        self.objects.create_intl_object(
            IntlValue::RelativeTimeFormat(Box::new(RelativeTimeFormatValue {
                locale,
                numbering_system,
                style,
                numeric,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_relative_time_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        to_parts: bool,
    ) -> Result<Value> {
        let formatter = self.relative_time_receiver(this_value)?;
        let number = self.to_number(args.as_slice().first().unwrap_or(&Value::Undefined))?;
        let unit_value = args.as_slice().get(1).unwrap_or(&Value::Undefined);
        let unit = self.to_string(unit_value)?;
        if !number.is_finite() {
            return Err(relative_time_range_error(
                "relative time value must be finite",
            ));
        }
        let unit = canonical_relative_time_unit(&unit)?;
        let parts = relative_time_parts(&formatter, number, unit);
        if !to_parts {
            let text = parts.into_iter().map(|part| part.value).collect::<String>();
            return self.heap_string_value(&text);
        }
        let mut values = Vec::with_capacity(parts.len());
        for part in parts {
            let kind = self.heap_string_value(part.kind)?;
            let value = self.heap_string_value(&part.value)?;
            let mut fields = vec![("type", kind), ("value", value)];
            if part.numeric {
                fields.push(("unit", self.heap_string_value(unit)?));
            }
            values.push(self.create_intl_data_object(fields)?);
        }
        self.create_array_from_elements(values)
    }

    pub(super) fn eval_intl_relative_time_format_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.relative_time_receiver(this_value)?;
        let locale = self.heap_string_value(&formatter.locale)?;
        let style = self.heap_string_value(&formatter.style)?;
        let numeric = self.heap_string_value(&formatter.numeric)?;
        let numbering_system = self.heap_string_value(&formatter.numbering_system)?;
        self.create_intl_data_object(vec![
            ("locale", locale),
            ("style", style),
            ("numeric", numeric),
            ("numberingSystem", numbering_system),
        ])
    }

    pub(super) fn eval_intl_relative_time_format_supported_locales(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let options_source = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options_source, Value::Null) {
            return Err(Error::type_error("Intl locale options cannot be null"));
        }
        let options = if matches!(options_source, Value::Undefined) {
            Value::Undefined
        } else {
            self.object_to_object(&options_source)?
        };
        let _matcher = self.relative_time_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let mut supported = Vec::new();
        for locale in locales {
            if relative_time_locale_is_supported(&locale) {
                supported.push(self.heap_string_value(&locale)?);
            }
        }
        self.create_array_from_elements(supported)
    }

    fn relative_time_receiver(&self, this_value: &Value) -> Result<RelativeTimeFormatValue> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error(
                "Intl.RelativeTimeFormat receiver is invalid",
            ));
        };
        let Some(IntlValue::RelativeTimeFormat(value)) = self.objects.intl_value(*id)? else {
            return Err(Error::type_error(
                "Intl.RelativeTimeFormat receiver is invalid",
            ));
        };
        Ok(value.as_ref().clone())
    }

    fn relative_time_string_option(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
        default: &str,
    ) -> Result<String> {
        let value = self.relative_time_option_value(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(default.to_owned());
        }
        let text = self.to_string(&value)?;
        if !allowed.contains(&text.as_str()) {
            return Err(relative_time_range_error(&format!(
                "{name} has an unsupported value"
            )));
        }
        Ok(text)
    }

    fn relative_time_optional_string(
        &mut self,
        options: &Value,
        name: &str,
    ) -> Result<Option<String>> {
        let value = self.relative_time_option_value(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        self.to_string(&value).map(Some)
    }

    fn relative_time_option_value(&mut self, options: &Value, name: &str) -> Result<Value> {
        if matches!(options, Value::Undefined) {
            return Ok(Value::Undefined);
        }
        self.get_named(options, name)
    }

    fn install_relative_time_static_method(&mut self, constructor: NativeFunctionId) -> Result<()> {
        let method = self.create_native_function(
            super::intl_kind(IntlFunctionKind::RelativeTimeFormatSupportedLocalesOf),
            Value::Undefined,
        )?;
        let key = self.intern_property_key(SUPPORTED_LOCALES_OF)?;
        self.define_native_function_property_key(
            constructor,
            SUPPORTED_LOCALES_OF,
            key,
            DataPropertyUpdate::new(
                Some(method),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )
    }
}

fn canonical_relative_time_unit(unit: &str) -> Result<&str> {
    if RELATIVE_TIME_UNITS.contains(&unit) {
        return Ok(unit);
    }
    if let Some(singular) = unit.strip_suffix('s')
        && RELATIVE_TIME_UNITS.contains(&singular)
    {
        return Ok(singular);
    }
    Err(relative_time_range_error(
        "relative time unit is unsupported",
    ))
}

const fn relative_time_locale_is_supported(locale: &str) -> bool {
    !locale.eq_ignore_ascii_case("zxx")
}

fn relative_time_range_error(message: &str) -> Error {
    Error::exception(ErrorName::RangeError, message)
}
