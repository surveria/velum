use num_traits::ToPrimitive;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{
            DataPropertyUpdate, DurationFormatValue, DurationUnitOptions, IntlValue,
            PropertyConfigurable, PropertyEnumerable, PropertyWritable,
        },
    },
    value::{ErrorName, NativeFunctionId, Value},
};

use super::{
    duration_formatting::format_duration_parts,
    number_options::{is_unicode_type, resolved_numbering_system},
};

const DURATION_FORMAT_TAG: &str = "Intl.DurationFormat";
const SUPPORTED_LOCALES_OF: &str = "supportedLocalesOf";
const DEFAULT_LOCALE: &str = "en-US";

pub(super) const DURATION_UNITS: [&str; 10] = [
    "years",
    "months",
    "weeks",
    "days",
    "hours",
    "minutes",
    "seconds",
    "milliseconds",
    "microseconds",
    "nanoseconds",
];

impl Context {
    pub(in crate::runtime) fn intl_duration_format_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::DurationFormatConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let constructor = self.intl_constructor_value(
            constructor_kind,
            DURATION_FORMAT_TAG,
            &[
                ("format", IntlFunctionKind::DurationFormatFormat),
                (
                    "formatToParts",
                    IntlFunctionKind::DurationFormatFormatToParts,
                ),
                (
                    "resolvedOptions",
                    IntlFunctionKind::DurationFormatResolvedOptions,
                ),
            ],
        )?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime(
                "Intl.DurationFormat constructor is not native",
            ));
        };
        self.install_duration_format_static_method(constructor_id)?;
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(super) fn construct_intl_duration_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let source = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(source, Value::Null) {
            return Err(Error::type_error(
                "Intl.DurationFormat options cannot be null",
            ));
        }
        let options = if matches!(source, Value::Undefined) {
            Value::Undefined
        } else {
            self.object_to_object(&source)?
        };
        let _matcher = self.duration_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            Some("best fit"),
        )?;
        let numbering_option = self.duration_optional_string(&options, "numberingSystem")?;
        if numbering_option
            .as_deref()
            .is_some_and(|value| !is_unicode_type(value))
        {
            return Err(duration_range_error("numberingSystem has an invalid value"));
        }
        let style = self
            .duration_string_option(
                &options,
                "style",
                &["long", "short", "narrow", "digital"],
                Some("short"),
            )?
            .ok_or_else(|| Error::runtime("DurationFormat style default is missing"))?;
        let units = self.duration_unit_options(&options, &style)?;
        let fractional_digits = self.duration_fractional_digits(&options)?;
        let requested_locale = locales
            .into_iter()
            .find(|locale| !locale.eq_ignore_ascii_case("zxx"))
            .unwrap_or_else(|| DEFAULT_LOCALE.to_owned());
        let (locale, numbering_system) =
            resolved_numbering_system(&requested_locale, numbering_option);
        let prototype =
            self.intl_constructor_prototype(IntlFunctionKind::DurationFormatConstructor)?;
        self.objects.create_intl_object(
            IntlValue::Duration(Box::new(DurationFormatValue {
                locale,
                numbering_system,
                style,
                units,
                fractional_digits,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_duration_format(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        to_parts: bool,
    ) -> Result<Value> {
        let formatter = self.duration_format_receiver(this_value)?;
        let duration = self.duration_from_value(args.as_slice().first())?;
        let parts = format_duration_parts(&formatter, &duration)?;
        if !to_parts {
            let text = parts.into_iter().map(|part| part.value).collect::<String>();
            return self.heap_string_value(&text);
        }
        let mut values = Vec::with_capacity(parts.len());
        for part in parts {
            let kind = self.heap_string_value(part.kind)?;
            let value = self.heap_string_value(&part.value)?;
            let mut fields = vec![("type", kind), ("value", value)];
            if let Some(unit) = part.unit {
                fields.push(("unit", self.heap_string_value(unit)?));
            }
            values.push(self.create_intl_data_object(fields)?);
        }
        self.create_array_from_elements(values)
    }

    pub(in crate::runtime::native) fn format_temporal_duration_locale_string(
        &mut self,
        value: &Value,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.duration_receiver(value)?;
        let formatter = self.construct_intl_duration_format(args)?;
        self.eval_intl_duration_format(
            RuntimeCallArgs::values(std::slice::from_ref(value)),
            &formatter,
            false,
        )
    }

    pub(super) fn eval_intl_duration_format_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.duration_format_receiver(this_value)?;
        let mut fields = vec![
            ("locale", self.heap_string_value(&formatter.locale)?),
            (
                "numberingSystem",
                self.heap_string_value(&formatter.numbering_system)?,
            ),
            ("style", self.heap_string_value(&formatter.style)?),
        ];
        for (index, unit) in formatter.units.iter().enumerate() {
            let Some(name) = DURATION_UNITS.get(index) else {
                return Err(Error::runtime("DurationFormat unit table is invalid"));
            };
            fields.push((*name, self.heap_string_value(&unit.style)?));
            fields.push((
                duration_display_name(index)?,
                self.heap_string_value(&unit.display)?,
            ));
        }
        if let Some(fractional_digits) = formatter.fractional_digits {
            fields.push((
                "fractionalDigits",
                Value::Number(f64::from(fractional_digits)),
            ));
        }
        self.create_intl_data_object(fields)
    }

    pub(super) fn eval_intl_duration_format_supported_locales(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let source = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(source, Value::Null) {
            return Err(Error::type_error("Intl locale options cannot be null"));
        }
        let options = if matches!(source, Value::Undefined) {
            Value::Undefined
        } else {
            self.object_to_object(&source)?
        };
        let _matcher = self.duration_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            Some("best fit"),
        )?;
        let mut supported = Vec::new();
        for locale in locales {
            if !locale.eq_ignore_ascii_case("zxx") {
                supported.push(self.heap_string_value(&locale)?);
            }
        }
        self.create_array_from_elements(supported)
    }

    fn duration_format_receiver(&self, value: &Value) -> Result<DurationFormatValue> {
        let Value::Object(id) = value else {
            return Err(Error::type_error("Intl.DurationFormat receiver is invalid"));
        };
        let Some(IntlValue::Duration(formatter)) = self.objects.intl_value(*id)? else {
            return Err(Error::type_error("Intl.DurationFormat receiver is invalid"));
        };
        Ok(formatter.as_ref().clone())
    }

    fn duration_unit_options(
        &mut self,
        options: &Value,
        base_style: &str,
    ) -> Result<[DurationUnitOptions; 10]> {
        let mut requested = Vec::with_capacity(DURATION_UNITS.len());
        for (index, name) in DURATION_UNITS.iter().enumerate() {
            let allowed = duration_unit_styles(index);
            let style = self.duration_string_option(options, name, allowed, None)?;
            let display_name = duration_display_name(index)?;
            let display =
                self.duration_string_option(options, display_name, &["auto", "always"], None)?;
            requested.push((style, display));
        }
        let mut numeric_started = false;
        let mut resolved = Vec::with_capacity(DURATION_UNITS.len());
        for (index, (requested_style, requested_display)) in requested.into_iter().enumerate() {
            let default_style = duration_unit_default_style(base_style, index, numeric_started);
            let style_was_requested = requested_style.is_some();
            let mut style = requested_style.unwrap_or_else(|| default_style.to_owned());
            if numeric_started && !matches!(style.as_str(), "numeric" | "2-digit") {
                return Err(duration_range_error(
                    "non-numeric unit style follows a numeric unit",
                ));
            }
            if numeric_started && index <= 6 && style == "numeric" {
                "2-digit".clone_into(&mut style);
            }
            numeric_started |= matches!(style.as_str(), "numeric" | "2-digit");
            let display = requested_display.unwrap_or_else(|| {
                if style_was_requested || matches!(style.as_str(), "numeric" | "2-digit") {
                    "always".to_owned()
                } else {
                    "auto".to_owned()
                }
            });
            resolved.push(DurationUnitOptions { style, display });
        }
        resolved
            .try_into()
            .map_err(|_| Error::runtime("DurationFormat unit option table has an invalid length"))
    }

    fn duration_fractional_digits(&mut self, options: &Value) -> Result<Option<u8>> {
        let value = self.duration_option_value(options, "fractionalDigits")?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        let number = self.to_number(&value)?;
        if !number.is_finite() || !(0.0..=9.0).contains(&number) {
            return Err(duration_range_error("fractionalDigits is out of range"));
        }
        number
            .floor()
            .to_u8()
            .map(Some)
            .ok_or_else(|| duration_range_error("fractionalDigits is out of range"))
    }

    fn duration_string_option(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
        default: Option<&str>,
    ) -> Result<Option<String>> {
        let value = self.duration_option_value(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(default.map(str::to_owned));
        }
        let text = self.to_string(&value)?;
        if !allowed.contains(&text.as_str()) {
            return Err(duration_range_error(&format!(
                "{name} has an unsupported value"
            )));
        }
        Ok(Some(text))
    }

    fn duration_optional_string(&mut self, options: &Value, name: &str) -> Result<Option<String>> {
        let value = self.duration_option_value(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        self.to_string(&value).map(Some)
    }

    fn duration_option_value(&mut self, options: &Value, name: &str) -> Result<Value> {
        if matches!(options, Value::Undefined) {
            return Ok(Value::Undefined);
        }
        self.get_named(options, name)
    }

    fn install_duration_format_static_method(
        &mut self,
        constructor: NativeFunctionId,
    ) -> Result<()> {
        let method = self.create_native_function(
            super::intl_kind(IntlFunctionKind::DurationFormatSupportedLocalesOf),
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

const fn duration_unit_styles(index: usize) -> &'static [&'static str] {
    match index {
        0..=3 => &["long", "short", "narrow"],
        4..=6 => &["long", "short", "narrow", "numeric", "2-digit"],
        _ => &["long", "short", "narrow", "numeric"],
    }
}

fn duration_unit_default_style(base_style: &str, index: usize, numeric_started: bool) -> &str {
    if numeric_started {
        return if index <= 6 { "2-digit" } else { "numeric" };
    }
    if base_style != "digital" {
        return base_style;
    }
    match index {
        0..=3 => "short",
        5..=6 => "2-digit",
        _ => "numeric",
    }
}

fn duration_display_name(index: usize) -> Result<&'static str> {
    [
        "yearsDisplay",
        "monthsDisplay",
        "weeksDisplay",
        "daysDisplay",
        "hoursDisplay",
        "minutesDisplay",
        "secondsDisplay",
        "millisecondsDisplay",
        "microsecondsDisplay",
        "nanosecondsDisplay",
    ]
    .get(index)
    .copied()
    .ok_or_else(|| Error::runtime("DurationFormat display table is invalid"))
}

fn duration_range_error(message: &str) -> Error {
    Error::exception(ErrorName::RangeError, message)
}
