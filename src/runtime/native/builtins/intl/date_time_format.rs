use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{DateTimeFormatValue, IntlValue, ObjectPropertyInit, PropertyEnumerable},
    },
    value::Value,
};

impl Context {
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

    pub(super) fn date_time_format_receiver(
        &self,
        this_value: &Value,
    ) -> Result<DateTimeFormatValue> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Intl.DateTimeFormat receiver is invalid"));
        };
        match self.objects.intl_value(*id)? {
            Some(IntlValue::DateTime(value)) => Ok(value.as_ref().clone()),
            Some(IntlValue::Duration | IntlValue::Number(_)) | None => {
                Err(Error::type_error("Intl.DateTimeFormat receiver is invalid"))
            }
        }
    }

    pub(super) fn eval_intl_date_time_format_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.date_time_format_receiver(this_value)?;
        let mut fields = vec![
            ("locale", self.heap_string_value(&formatter.locale)?),
            ("calendar", self.heap_string_value(&formatter.calendar)?),
            ("numberingSystem", self.heap_string_value("latn")?),
            ("timeZone", self.heap_string_value(&formatter.time_zone)?),
        ];
        if let Some(hour_cycle) = Self::resolved_hour_cycle(&formatter) {
            fields.push(("hourCycle", self.heap_string_value(hour_cycle)?));
            fields.push(("hour12", Value::Bool(matches!(hour_cycle, "h11" | "h12"))));
        }
        for (name, value) in [
            ("dateStyle", formatter.options.date_style.as_deref()),
            ("timeStyle", formatter.options.time_style.as_deref()),
            ("weekday", formatter.options.weekday.as_deref()),
            ("era", formatter.options.era.as_deref()),
            ("year", formatter.options.year.as_deref()),
            ("month", formatter.options.month.as_deref()),
            ("day", formatter.options.day.as_deref()),
            ("hour", formatter.options.hour.as_deref()),
            ("minute", formatter.options.minute.as_deref()),
            ("second", formatter.options.second.as_deref()),
            ("dayPeriod", formatter.options.day_period.as_deref()),
            ("timeZoneName", formatter.options.time_zone_name.as_deref()),
        ] {
            if let Some(value) = value {
                fields.push((name, self.heap_string_value(value)?));
            }
        }
        if let Some(digits) = formatter.options.fractional_second_digits {
            fields.push(("fractionalSecondDigits", Value::Number(f64::from(digits))));
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
            "numberingSystem" => &["latn"],
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
        if !formatter.options.has_explicit_time_fields() && formatter.options.time_style.is_none() {
            return None;
        }
        if let Some(hour12) = formatter.options.hour12 {
            return Some(if hour12 { "h12" } else { "h23" });
        }
        if let Some(hour_cycle) = formatter.options.hour_cycle.as_deref() {
            return Some(hour_cycle);
        }
        Some(if formatter.locale.to_ascii_lowercase().starts_with("de") {
            "h23"
        } else {
            "h12"
        })
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
}
