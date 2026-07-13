use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{DateTimeFormatValue, IntlValue, ObjectPropertyInit, PropertyEnumerable},
    },
    value::{NativeFunctionId, ObjectId, Value},
};

use crate::runtime::object::{
    AccessorPropertyUpdate, DataPropertyUpdate, PropertyConfigurable, PropertyKey, PropertyLookup,
    PropertyUpdate, PropertyWritable,
};

const DATE_TIME_FORMAT_TAG: &str = "Intl.DateTimeFormat";
const SUPPORTED_LOCALES_OF: &str = "supportedLocalesOf";
const LEGACY_CONSTRUCTED_SYMBOL: &str = "IntlLegacyConstructedSymbol";

impl Context {
    pub(in crate::runtime) fn intl_date_time_format_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::DateTimeFormatConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let constructor = self.intl_constructor_value(
            constructor_kind,
            DATE_TIME_FORMAT_TAG,
            &[
                (
                    "formatToParts",
                    IntlFunctionKind::DateTimeFormatFormatToParts,
                ),
                (
                    "resolvedOptions",
                    IntlFunctionKind::DateTimeFormatResolvedOptions,
                ),
                ("formatRange", IntlFunctionKind::DateTimeFormatFormatRange),
                (
                    "formatRangeToParts",
                    IntlFunctionKind::DateTimeFormatFormatRangeToParts,
                ),
            ],
        )?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime(
                "Intl.DateTimeFormat constructor is not native",
            ));
        };
        let prototype = self.date_time_format_prototype_id(constructor_id)?;
        self.install_date_time_format_accessor(prototype)?;
        self.install_date_time_format_static_methods(constructor_id)?;
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(super) fn construct_intl_date_time_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let value = self.parse_date_time_format(args)?;
        let prototype =
            self.intl_constructor_prototype(IntlFunctionKind::DateTimeFormatConstructor)?;
        self.objects.create_intl_object(
            IntlValue::DateTime(Box::new(value)),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn call_intl_date_time_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let prototype =
            self.intl_constructor_prototype(IntlFunctionKind::DateTimeFormatConstructor)?;
        let legacy_receiver = match this_value {
            Value::Object(id) => {
                *id == prototype || self.objects.prototype_chain_has_object(*id, prototype)?
            }
            _ => false,
        };
        let date_time_format = self.construct_intl_date_time_format(args)?;
        if !legacy_receiver {
            return Ok(date_time_format);
        }
        let Value::Object(receiver) = this_value else {
            return Err(Error::runtime("legacy DateTimeFormat receiver disappeared"));
        };
        let symbol = self.create_symbol_value(Some(LEGACY_CONSTRUCTED_SYMBOL))?;
        let Value::Symbol(symbol) = symbol else {
            return Err(Error::runtime("legacy DateTimeFormat symbol is invalid"));
        };
        self.objects.define_property(
            *receiver,
            PropertyKey::symbol(symbol.id()),
            LEGACY_CONSTRUCTED_SYMBOL,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(date_time_format),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )?;
        Ok(this_value.clone())
    }

    pub(super) fn eval_intl_date_time_format_getter(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter_id = self.date_time_format_receiver_id(this_value)?;
        let cached = match self.objects.intl_value(formatter_id)? {
            Some(IntlValue::DateTime(value)) => value.bound_format.clone(),
            _ => {
                return Err(Error::type_error("Intl.DateTimeFormat receiver is invalid"));
            }
        };
        if let Some(cached) = cached {
            return Ok(cached);
        }
        let bound = self.create_ephemeral_native_function(
            super::intl_kind(IntlFunctionKind::DateTimeFormatBoundFormat(formatter_id)),
            Value::Undefined,
        )?;
        let Some(IntlValue::DateTime(value)) = self.objects.intl_value_mut(formatter_id)? else {
            return Err(Error::runtime("Intl.DateTimeFormat receiver disappeared"));
        };
        value.bound_format = Some(bound.clone());
        Ok(bound)
    }

    pub(super) fn eval_intl_date_time_format_bound(
        &mut self,
        args: RuntimeCallArgs<'_>,
        formatter: ObjectId,
    ) -> Result<Value> {
        self.eval_intl_date_time_format(args, &Value::Object(formatter), false)
    }

    pub(super) fn date_time_format_receiver(
        &mut self,
        this_value: &Value,
    ) -> Result<DateTimeFormatValue> {
        let id = self.date_time_format_receiver_id(this_value)?;
        match self.objects.intl_value(id)? {
            Some(IntlValue::DateTime(value)) => Ok(value.as_ref().clone()),
            Some(
                IntlValue::Collator(_)
                | IntlValue::Duration
                | IntlValue::DisplayNames(_)
                | IntlValue::List(_)
                | IntlValue::Locale(_)
                | IntlValue::Number(_)
                | IntlValue::PluralRules(_)
                | IntlValue::RelativeTimeFormat(_)
                | IntlValue::Segmenter(_)
                | IntlValue::Segments(_)
                | IntlValue::SegmentIterator(_),
            )
            | None => Err(Error::type_error("Intl.DateTimeFormat receiver is invalid")),
        }
    }

    fn date_time_format_receiver_id(&mut self, this_value: &Value) -> Result<ObjectId> {
        if let Value::Object(id) = this_value
            && matches!(self.objects.intl_value(*id)?, Some(IntlValue::DateTime(_)))
        {
            return Ok(*id);
        }
        for key in self.semantic_own_property_keys(this_value)? {
            let Value::Symbol(symbol) = key else {
                continue;
            };
            if symbol.description() != Some(LEGACY_CONSTRUCTED_SYMBOL) {
                continue;
            }
            let lookup = PropertyLookup::from_key(
                LEGACY_CONSTRUCTED_SYMBOL,
                PropertyKey::symbol(symbol.id()),
            );
            let legacy_target = self.get(this_value, lookup)?;
            let Value::Object(id) = legacy_target else {
                continue;
            };
            if matches!(self.objects.intl_value(id)?, Some(IntlValue::DateTime(_))) {
                return Ok(id);
            }
        }
        Err(Error::type_error("Intl.DateTimeFormat receiver is invalid"))
    }

    pub(super) fn eval_intl_date_time_format_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.date_time_format_receiver(this_value)?;
        let mut fields = vec![
            ("locale", self.heap_string_value(&formatter.locale)?),
            ("calendar", self.heap_string_value(&formatter.calendar)?),
            (
                "numberingSystem",
                self.heap_string_value(&formatter.numbering_system)?,
            ),
            ("timeZone", self.heap_string_value(&formatter.time_zone)?),
        ];
        if let Some(hour_cycle) = Self::resolved_hour_cycle(&formatter) {
            fields.push(("hourCycle", self.heap_string_value(hour_cycle)?));
            fields.push(("hour12", Value::Bool(matches!(hour_cycle, "h11" | "h12"))));
        }
        for (name, value) in [
            ("weekday", formatter.options.weekday.as_deref()),
            ("era", formatter.options.era.as_deref()),
            ("year", formatter.options.year.as_deref()),
            ("month", formatter.options.month.as_deref()),
            ("day", formatter.options.day.as_deref()),
            ("dayPeriod", formatter.options.day_period.as_deref()),
            ("hour", formatter.options.hour.as_deref()),
            ("minute", formatter.options.minute.as_deref()),
            ("second", formatter.options.second.as_deref()),
        ] {
            if let Some(value) = value {
                fields.push((name, self.heap_string_value(value)?));
            }
        }
        if let Some(digits) = formatter.options.fractional_second_digits {
            fields.push(("fractionalSecondDigits", Value::Number(f64::from(digits))));
        }
        for (name, value) in [
            ("timeZoneName", formatter.options.time_zone_name.as_deref()),
            ("dateStyle", formatter.options.date_style.as_deref()),
            ("timeStyle", formatter.options.time_style.as_deref()),
        ] {
            if let Some(value) = value {
                fields.push((name, self.heap_string_value(value)?));
            }
        }
        self.create_intl_data_object(fields)
    }

    pub(super) fn eval_intl_supported_values_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let key = self.to_string(args.as_slice().first().unwrap_or(&Value::Undefined))?;
        let values: &[&str] = match key.as_str() {
            "calendar" => &[
                "buddhist",
                "chinese",
                "coptic",
                "dangi",
                "ethioaa",
                "ethiopic",
                "gregory",
                "hebrew",
                "indian",
                "islamic-civil",
                "islamic-tbla",
                "islamic-umalqura",
                "iso8601",
                "japanese",
                "persian",
                "roc",
            ],
            "timeZone" => &[
                "Africa/Monrovia",
                "America/Los_Angeles",
                "America/New_York",
                "Asia/Kolkata",
                "Europe/Berlin",
                "Europe/Vienna",
                "Pacific/Apia",
                "UTC",
            ],
            "numberingSystem" => super::number_digits::SUPPORTED_NUMBERING_SYSTEMS,
            _ => {
                return Err(Error::exception(
                    crate::value::ErrorName::RangeError,
                    "Intl.supportedValuesOf key is unsupported",
                ));
            }
        };
        let mut result = Vec::with_capacity(values.len());
        for value in values {
            result.push(self.heap_string_value(value)?);
        }
        self.create_array_from_elements(result)
    }

    pub(super) fn resolved_hour_cycle(formatter: &DateTimeFormatValue) -> Option<&str> {
        if formatter.options.hour.is_none() && formatter.options.time_style.is_none() {
            return None;
        }
        formatter.options.hour_cycle.as_deref()
    }

    pub(super) fn create_intl_data_object(
        &mut self,
        fields: Vec<(&'static str, Value)>,
    ) -> Result<Value> {
        let mut properties = Vec::with_capacity(fields.len());
        for (name, value) in fields {
            let key = self.intern_property_key(name)?;
            properties.push(ObjectPropertyInit::new(
                key,
                name,
                value,
                PropertyEnumerable::Yes,
            ));
        }
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            properties,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn date_time_format_prototype_id(&self, constructor: NativeFunctionId) -> Result<ObjectId> {
        match self.native_function(constructor)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime(
                "Intl.DateTimeFormat prototype is not an object",
            )),
        }
    }

    fn install_date_time_format_accessor(&mut self, prototype: ObjectId) -> Result<()> {
        let getter = self.create_native_function(
            super::intl_kind(IntlFunctionKind::DateTimeFormatFormatGetter),
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

    fn install_date_time_format_static_methods(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        let method = self.create_native_function(
            super::intl_kind(IntlFunctionKind::DateTimeFormatSupportedLocalesOf),
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
