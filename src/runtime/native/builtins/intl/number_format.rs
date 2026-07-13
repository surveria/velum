use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, IntlValue, NumberFormatValue,
            PropertyConfigurable, PropertyEnumerable, PropertyUpdate, PropertyWritable,
        },
    },
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

const NUMBER_FORMAT_TAG: &str = "Intl.NumberFormat";
const SUPPORTED_LOCALES_OF: &str = "supportedLocalesOf";

impl Context {
    pub(super) fn intl_number_format_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::NumberFormatConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let constructor = self.intl_constructor_value(
            constructor_kind,
            NUMBER_FORMAT_TAG,
            &[
                ("formatToParts", IntlFunctionKind::NumberFormatFormatToParts),
                (
                    "resolvedOptions",
                    IntlFunctionKind::NumberFormatResolvedOptions,
                ),
                ("formatRange", IntlFunctionKind::NumberFormatFormatRange),
                (
                    "formatRangeToParts",
                    IntlFunctionKind::NumberFormatFormatRangeToParts,
                ),
            ],
        )?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime(
                "Intl.NumberFormat constructor is not native",
            ));
        };
        let prototype = self.number_format_prototype_id(constructor_id)?;
        self.install_number_format_accessor(prototype)?;
        self.install_number_format_static_methods(constructor_id)?;
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(super) fn construct_intl_number_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = self.parse_number_format(args)?;
        let prototype =
            self.intl_constructor_prototype(IntlFunctionKind::NumberFormatConstructor)?;
        self.objects.create_intl_object(
            IntlValue::NumberFormat(Box::new(value)),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_number_format_getter(&mut self, this_value: &Value) -> Result<Value> {
        let Value::Object(formatter_id) = this_value else {
            return Err(Error::type_error("Intl.NumberFormat receiver is invalid"));
        };
        let cached = match self.objects.intl_value(*formatter_id)? {
            Some(IntlValue::NumberFormat(value)) => value.bound_format.clone(),
            _ => return Err(Error::type_error("Intl.NumberFormat receiver is invalid")),
        };
        if let Some(cached) = cached {
            return Ok(cached);
        }
        let bound = self.create_ephemeral_native_function(
            super::intl_kind(IntlFunctionKind::NumberFormatBoundFormat(*formatter_id)),
            Value::Undefined,
        )?;
        let Some(IntlValue::NumberFormat(value)) = self.objects.intl_value_mut(*formatter_id)?
        else {
            return Err(Error::runtime("Intl.NumberFormat receiver disappeared"));
        };
        value.bound_format = Some(bound.clone());
        Ok(bound)
    }

    pub(super) fn number_format_receiver(&self, this_value: &Value) -> Result<NumberFormatValue> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Intl.NumberFormat receiver is invalid"));
        };
        match self.objects.intl_value(*id)? {
            Some(IntlValue::NumberFormat(value)) => Ok(value.as_ref().clone()),
            _ => Err(Error::type_error("Intl.NumberFormat receiver is invalid")),
        }
    }

    pub(super) fn eval_intl_number_format_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.number_format_receiver(this_value)?;
        let mut fields = vec![
            ("locale", self.heap_string_value(&formatter.locale)?),
            (
                "numberingSystem",
                self.heap_string_value(&formatter.numbering_system)?,
            ),
            ("style", self.heap_string_value(&formatter.style)?),
        ];
        if let Some(currency) = &formatter.currency {
            fields.push(("currency", self.heap_string_value(currency)?));
            fields.push((
                "currencyDisplay",
                self.heap_string_value(&formatter.currency_display)?,
            ));
            fields.push((
                "currencySign",
                self.heap_string_value(&formatter.currency_sign)?,
            ));
        }
        if let Some(unit) = &formatter.unit {
            fields.push(("unit", self.heap_string_value(unit)?));
            fields.push((
                "unitDisplay",
                self.heap_string_value(&formatter.unit_display)?,
            ));
        }
        fields.push((
            "minimumIntegerDigits",
            Value::Number(f64::from(formatter.minimum_integer_digits)),
        ));
        if let (Some(minimum), Some(maximum)) = (
            formatter.minimum_significant_digits,
            formatter.maximum_significant_digits,
        ) {
            fields.push((
                "minimumSignificantDigits",
                Value::Number(f64::from(minimum)),
            ));
            fields.push((
                "maximumSignificantDigits",
                Value::Number(f64::from(maximum)),
            ));
        } else {
            fields.push((
                "minimumFractionDigits",
                Value::Number(f64::from(formatter.minimum_fraction_digits)),
            ));
            fields.push((
                "maximumFractionDigits",
                Value::Number(f64::from(formatter.maximum_fraction_digits)),
            ));
        }
        fields.push((
            "useGrouping",
            match &formatter.use_grouping {
                Some(value) => self.heap_string_value(value)?,
                None => Value::Bool(false),
            },
        ));
        fields.push(("notation", self.heap_string_value(&formatter.notation)?));
        if formatter.notation == "compact" {
            fields.push((
                "compactDisplay",
                self.heap_string_value(&formatter.compact_display)?,
            ));
        }
        fields.push((
            "signDisplay",
            self.heap_string_value(&formatter.sign_display)?,
        ));
        fields.push((
            "roundingIncrement",
            Value::Number(f64::from(formatter.rounding_increment)),
        ));
        fields.push((
            "roundingMode",
            self.heap_string_value(&formatter.rounding_mode)?,
        ));
        fields.push((
            "roundingPriority",
            self.heap_string_value(&formatter.rounding_priority)?,
        ));
        fields.push((
            "trailingZeroDisplay",
            self.heap_string_value(&formatter.trailing_zero_display)?,
        ));
        self.create_intl_data_object(fields)
    }

    pub(super) fn eval_intl_number_format_supported_locales(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options, Value::Null) {
            return Err(Error::type_error("locale options cannot be null"));
        }
        if !matches!(options, Value::Undefined) {
            let matcher = self.get_named(&options, "localeMatcher")?;
            if !matches!(matcher, Value::Undefined) {
                let matcher = self.to_string(&matcher)?;
                if !matches!(matcher.as_str(), "lookup" | "best fit") {
                    return Err(Error::exception(
                        ErrorName::RangeError,
                        "localeMatcher has an unsupported value",
                    ));
                }
            }
        }
        let locales = self.number_format_locale_list(&requested)?;
        let mut supported = Vec::new();
        for locale in locales {
            if locale.eq_ignore_ascii_case("zxx") {
                continue;
            }
            supported.push(self.heap_string_value(&canonical_locale(&locale)?)?);
        }
        self.create_array_from_elements(supported)
    }

    fn number_format_locale_list(&mut self, value: &Value) -> Result<Vec<String>> {
        if matches!(value, Value::Undefined) {
            return Ok(Vec::new());
        }
        if value.string_text().is_some() {
            return Ok(vec![self.to_string(value)?]);
        }
        let Value::Object(_) = value else {
            return Err(Error::type_error("Intl locale list is invalid"));
        };
        let length_value = self.get_named(value, "length")?;
        let length = Self::length_to_usize(
            self.to_length(&length_value)?,
            "Intl locale list length exceeded supported range",
        )?;
        let mut locales = Vec::new();
        for index in 0..length {
            self.step()?;
            let item = self.get_named(value, &index.to_string())?;
            if matches!(item, Value::Undefined) {
                continue;
            }
            let locale = self.to_string(&item)?;
            let locale = canonical_locale(&locale)?;
            if !locales.contains(&locale) {
                locales.push(locale);
            }
        }
        Ok(locales)
    }

    fn number_format_prototype_id(&self, constructor: NativeFunctionId) -> Result<ObjectId> {
        match self.native_function(constructor)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime(
                "Intl.NumberFormat prototype is not an object",
            )),
        }
    }

    fn install_number_format_accessor(&mut self, prototype: ObjectId) -> Result<()> {
        let getter = self.create_native_function(
            super::intl_kind(IntlFunctionKind::NumberFormatFormatGetter),
            Value::Undefined,
        )?;
        let key = self.intern_property_key("format")?;
        self.objects.define_property(
            prototype,
            key,
            "format",
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn install_number_format_static_methods(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        let method = self.create_native_function(
            super::intl_kind(IntlFunctionKind::NumberFormatSupportedLocalesOf),
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

fn canonical_locale(locale: &str) -> Result<String> {
    if locale.is_empty()
        || locale
            .split('-')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    {
        return Err(Error::exception(
            ErrorName::RangeError,
            "Intl locale is invalid",
        ));
    }
    let mut parts = locale.split('-');
    let Some(language) = parts.next() else {
        return Err(Error::exception(
            ErrorName::RangeError,
            "Intl locale is invalid",
        ));
    };
    if !(2..=8).contains(&language.len())
        || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return Err(Error::exception(
            ErrorName::RangeError,
            "Intl locale is invalid",
        ));
    }
    let mut result = language.to_ascii_lowercase();
    for part in parts {
        result.push('-');
        if part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            result.push_str(&part.to_ascii_uppercase());
        } else {
            result.push_str(&part.to_ascii_lowercase());
        }
    }
    Ok(result)
}
