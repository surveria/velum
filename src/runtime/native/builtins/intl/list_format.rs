use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::IteratorStep,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{
            DataPropertyUpdate, IntlValue, ListFormatValue, PropertyConfigurable,
            PropertyEnumerable, PropertyWritable,
        },
    },
    value::{ErrorName, NativeFunctionId, Value},
};

const LIST_FORMAT_TAG: &str = "Intl.ListFormat";
const SUPPORTED_LOCALES_OF: &str = "supportedLocalesOf";
const DEFAULT_LOCALE: &str = "en-US";

struct ListPart {
    kind: &'static str,
    value: String,
}

struct ListPattern {
    middle: &'static str,
    pair: &'static str,
    end: &'static str,
}

impl Context {
    pub(in crate::runtime) fn intl_list_format_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::ListFormatConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let constructor = self.intl_constructor_value(
            constructor_kind,
            LIST_FORMAT_TAG,
            &[
                ("format", IntlFunctionKind::ListFormatFormat),
                ("formatToParts", IntlFunctionKind::ListFormatFormatToParts),
                (
                    "resolvedOptions",
                    IntlFunctionKind::ListFormatResolvedOptions,
                ),
            ],
        )?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime("Intl.ListFormat constructor is not native"));
        };
        self.install_list_format_static_method(constructor_id)?;
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(super) fn construct_intl_list_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.list_format_locale_list(&requested)?;
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if !matches!(options, Value::Undefined) && !is_object_value(&options) {
            return Err(Error::type_error(
                "Intl.ListFormat options must be an object",
            ));
        }
        let _matcher = self.list_format_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let list_type = self.list_format_option(
            &options,
            "type",
            &["conjunction", "disjunction", "unit"],
            "conjunction",
        )?;
        let style =
            self.list_format_option(&options, "style", &["long", "short", "narrow"], "long")?;
        let locale = locales
            .into_iter()
            .find(|locale| !locale.eq_ignore_ascii_case("zxx"))
            .unwrap_or_else(|| DEFAULT_LOCALE.to_owned());
        let prototype = self.intl_constructor_prototype(IntlFunctionKind::ListFormatConstructor)?;
        self.objects.create_intl_object(
            IntlValue::List(Box::new(ListFormatValue {
                locale,
                list_type,
                style,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_list_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        to_parts: bool,
    ) -> Result<Value> {
        let formatter = self.list_format_receiver(this_value)?;
        let values = self.list_format_strings(args.as_slice().first())?;
        let parts = list_parts(&formatter, &values);
        if !to_parts {
            let text = parts.into_iter().map(|part| part.value).collect::<String>();
            return self.heap_string_value(&text);
        }
        let mut result = Vec::with_capacity(parts.len());
        for part in parts {
            let kind = self.heap_string_value(part.kind)?;
            let value = self.heap_string_value(&part.value)?;
            result.push(self.create_intl_data_object(vec![("type", kind), ("value", value)])?);
        }
        self.create_array_from_elements(result)
    }

    pub(super) fn eval_intl_list_format_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.list_format_receiver(this_value)?;
        let locale = self.heap_string_value(&formatter.locale)?;
        let list_type = self.heap_string_value(&formatter.list_type)?;
        let style = self.heap_string_value(&formatter.style)?;
        self.create_intl_data_object(vec![
            ("locale", locale),
            ("type", list_type),
            ("style", style),
        ])
    }

    pub(super) fn eval_intl_list_format_supported_locales(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.list_format_locale_list(&requested)?;
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options, Value::Null) {
            return Err(Error::type_error("locale options cannot be null"));
        }
        let _matcher = self.list_format_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let mut supported = Vec::new();
        for locale in locales {
            if !locale.eq_ignore_ascii_case("zxx") {
                supported.push(self.heap_string_value(&locale)?);
            }
        }
        self.create_array_from_elements(supported)
    }

    fn list_format_receiver(&self, this_value: &Value) -> Result<ListFormatValue> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Intl.ListFormat receiver is invalid"));
        };
        let Some(IntlValue::List(value)) = self.objects.intl_value(*id)? else {
            return Err(Error::type_error("Intl.ListFormat receiver is invalid"));
        };
        Ok(value.as_ref().clone())
    }

    fn list_format_strings(&mut self, iterable: Option<&Value>) -> Result<Vec<String>> {
        let Some(iterable) = iterable.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(Vec::new());
        };
        let mut source = self.get_iterator(iterable)?;
        let mut strings = Vec::new();
        loop {
            self.step()?;
            match self.iterator_step(&mut source)? {
                IteratorStep::Value(value) => {
                    let Some(text) = value.string_text() else {
                        let error =
                            Error::type_error("Intl.ListFormat iterable value is not a string");
                        return Err(self.iterator_close_on_error(&mut source, error));
                    };
                    strings.push(text.to_owned());
                }
                IteratorStep::Done => return Ok(strings),
                IteratorStep::Abrupt(completion) => {
                    return completion.into_result().map(|_| strings);
                }
            }
        }
    }

    fn list_format_locale_list(&mut self, value: &Value) -> Result<Vec<String>> {
        if matches!(value, Value::Undefined) {
            return Ok(Vec::new());
        }
        if value.string_text().is_some() || self.is_intl_locale_value(value)? {
            let tag = self.locale_source_tag(value)?;
            return Ok(vec![super::locale::canonicalize_locale_tag(&tag)?]);
        }
        if matches!(value, Value::Null) {
            return Err(Error::type_error("Intl locale list is invalid"));
        }
        let length_value = self.get_named(value, "length")?;
        let length = Self::length_to_usize(
            self.to_length(&length_value)?,
            "Intl locale list length exceeded supported range",
        )?;
        let mut locales = Vec::new();
        for index in 0..length {
            self.step()?;
            let name = index.to_string();
            let lookup = self.property_lookup(&name);
            if !self.has_property_value_with_lookup(value, lookup)? {
                continue;
            }
            let item = self.get_named(value, &name)?;
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

    fn list_format_option(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
        default: &str,
    ) -> Result<String> {
        if matches!(options, Value::Undefined) {
            return Ok(default.to_owned());
        }
        let value = self.get_named(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(default.to_owned());
        }
        let text = self.to_string(&value)?;
        if !allowed.contains(&text.as_str()) {
            return Err(Error::exception(
                ErrorName::RangeError,
                format!("{name} has an unsupported value"),
            ));
        }
        Ok(text)
    }

    fn install_list_format_static_method(&mut self, constructor: NativeFunctionId) -> Result<()> {
        let method = self.create_native_function(
            super::intl_kind(IntlFunctionKind::ListFormatSupportedLocalesOf),
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

const fn is_object_value(value: &Value) -> bool {
    matches!(
        value,
        Value::Object(_) | Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_)
    )
}

fn list_parts(formatter: &ListFormatValue, values: &[String]) -> Vec<ListPart> {
    let mut parts = Vec::with_capacity(values.len().saturating_mul(2).saturating_sub(1));
    let pattern = list_pattern(formatter, values.len());
    for (index, value) in values.iter().enumerate() {
        parts.push(ListPart {
            kind: "element",
            value: value.clone(),
        });
        let Some(next) = index.checked_add(1) else {
            continue;
        };
        if next >= values.len() {
            continue;
        }
        let separator = if values.len() == 2 {
            pattern.pair
        } else if next == values.len().saturating_sub(1) {
            pattern.end
        } else {
            pattern.middle
        };
        parts.push(ListPart {
            kind: "literal",
            value: separator.to_owned(),
        });
    }
    parts
}

fn list_pattern(formatter: &ListFormatValue, count: usize) -> ListPattern {
    let spanish = formatter.locale.to_ascii_lowercase().starts_with("es");
    match (
        formatter.list_type.as_str(),
        formatter.style.as_str(),
        spanish,
        count,
    ) {
        ("unit", "narrow", _, _) => ListPattern {
            middle: " ",
            pair: " ",
            end: " ",
        },
        ("unit", "long", true, _) | ("conjunction", _, true, _) => ListPattern {
            middle: ", ",
            pair: " y ",
            end: " y ",
        },
        ("unit", "short", true, 2) => ListPattern {
            middle: ", ",
            pair: " y ",
            end: ", ",
        },
        ("unit", _, _, _) => ListPattern {
            middle: ", ",
            pair: ", ",
            end: ", ",
        },
        ("disjunction", _, true, _) => ListPattern {
            middle: ", ",
            pair: " o ",
            end: " o ",
        },
        ("disjunction", _, false, _) => ListPattern {
            middle: ", ",
            pair: " or ",
            end: ", or ",
        },
        ("conjunction", "short", false, _) => ListPattern {
            middle: ", ",
            pair: " & ",
            end: ", & ",
        },
        _ => ListPattern {
            middle: ", ",
            pair: " and ",
            end: ", and ",
        },
    }
}
