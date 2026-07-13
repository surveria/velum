use num_traits::ToPrimitive;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{
            DataPropertyUpdate, IntlValue, PluralRulesValue, PropertyConfigurable,
            PropertyEnumerable, PropertyWritable,
        },
    },
    value::{ErrorName, NativeFunctionId, Value},
};

const PLURAL_RULES_TAG: &str = "Intl.PluralRules";
const SUPPORTED_LOCALES_OF: &str = "supportedLocalesOf";
const DEFAULT_LOCALE: &str = "en-US";
const ROUNDING_INCREMENTS: &[u16] = &[
    1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 2500, 5000,
];

struct PluralDigitOptions {
    minimum_integer: u8,
    minimum_fraction: u8,
    maximum_fraction: u8,
    minimum_significant: Option<u8>,
    maximum_significant: Option<u8>,
    rounding_increment: u16,
    rounding_mode: String,
    rounding_priority: String,
    trailing_zero_display: String,
}

impl Context {
    pub(in crate::runtime) fn intl_plural_rules_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::PluralRulesConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let constructor = self.intl_constructor_value(
            constructor_kind,
            PLURAL_RULES_TAG,
            &[
                ("select", IntlFunctionKind::PluralRulesSelect),
                ("selectRange", IntlFunctionKind::PluralRulesSelectRange),
                (
                    "resolvedOptions",
                    IntlFunctionKind::PluralRulesResolvedOptions,
                ),
            ],
        )?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime("Intl.PluralRules constructor is not native"));
        };
        self.install_plural_rules_static_method(constructor_id)?;
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(super) fn construct_intl_plural_rules(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let options_source = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if matches!(options_source, Value::Null) {
            return Err(Error::type_error("Intl.PluralRules options cannot be null"));
        }
        let options = if matches!(options_source, Value::Undefined) {
            Value::Undefined
        } else {
            self.object_to_object(&options_source)?
        };
        let _matcher = self.plural_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let rule_type =
            self.plural_string_option(&options, "type", &["cardinal", "ordinal"], "cardinal")?;
        let notation = self.plural_string_option(
            &options,
            "notation",
            &["standard", "scientific", "engineering", "compact"],
            "standard",
        )?;
        let compact_display_value =
            self.plural_string_option(&options, "compactDisplay", &["short", "long"], "short")?;
        let digits = self.parse_plural_digit_options(&options)?;
        let compact_display = (notation == "compact").then_some(compact_display_value);
        let locale = locales
            .into_iter()
            .find(|locale| plural_locale_is_supported(locale))
            .unwrap_or_else(|| DEFAULT_LOCALE.to_owned());
        let prototype =
            self.intl_constructor_prototype(IntlFunctionKind::PluralRulesConstructor)?;
        self.objects.create_intl_object(
            IntlValue::PluralRules(Box::new(PluralRulesValue {
                locale,
                rule_type,
                notation,
                compact_display,
                minimum_integer_digits: digits.minimum_integer,
                minimum_fraction_digits: digits.minimum_fraction,
                maximum_fraction_digits: digits.maximum_fraction,
                minimum_significant_digits: digits.minimum_significant,
                maximum_significant_digits: digits.maximum_significant,
                rounding_increment: digits.rounding_increment,
                rounding_mode: digits.rounding_mode,
                rounding_priority: digits.rounding_priority,
                trailing_zero_display: digits.trailing_zero_display,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_plural_rules_select(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let rules = self.plural_rules_receiver(this_value)?;
        let number = self.to_number(args.as_slice().first().unwrap_or(&Value::Undefined))?;
        let category = plural_category(&rules.locale, &rules.rule_type, number, &rules.notation);
        self.heap_string_value(category)
    }

    pub(super) fn eval_intl_plural_rules_select_range(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let _rules = self.plural_rules_receiver(this_value)?;
        let start = args.as_slice().first().unwrap_or(&Value::Undefined);
        let end = args.as_slice().get(1).unwrap_or(&Value::Undefined);
        if matches!(start, Value::Undefined) || matches!(end, Value::Undefined) {
            return Err(Error::type_error(
                "Intl.PluralRules.selectRange requires two values",
            ));
        }
        let start = self.to_number(start)?;
        let end = self.to_number(end)?;
        if start.is_nan() || end.is_nan() {
            return Err(plural_range_error("selectRange value is NaN"));
        }
        self.heap_string_value("other")
    }

    pub(super) fn eval_intl_plural_rules_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let rules = self.plural_rules_receiver(this_value)?;
        let categories = plural_categories(&rules.locale, &rules.rule_type);
        let mut category_values = Vec::with_capacity(categories.len());
        for category in categories {
            category_values.push(self.heap_string_value(category)?);
        }
        let categories = self.create_array_from_elements(category_values)?;
        let mut fields = vec![
            ("locale", self.heap_string_value(&rules.locale)?),
            ("type", self.heap_string_value(&rules.rule_type)?),
            ("notation", self.heap_string_value(&rules.notation)?),
        ];
        if let Some(compact_display) = &rules.compact_display {
            fields.push(("compactDisplay", self.heap_string_value(compact_display)?));
        }
        fields.push((
            "minimumIntegerDigits",
            Value::Number(f64::from(rules.minimum_integer_digits)),
        ));
        fields.push((
            "minimumFractionDigits",
            Value::Number(f64::from(rules.minimum_fraction_digits)),
        ));
        fields.push((
            "maximumFractionDigits",
            Value::Number(f64::from(rules.maximum_fraction_digits)),
        ));
        if let Some(minimum) = rules.minimum_significant_digits {
            fields.push((
                "minimumSignificantDigits",
                Value::Number(f64::from(minimum)),
            ));
        }
        if let Some(maximum) = rules.maximum_significant_digits {
            fields.push((
                "maximumSignificantDigits",
                Value::Number(f64::from(maximum)),
            ));
        }
        fields.push(("pluralCategories", categories));
        fields.push((
            "roundingIncrement",
            Value::Number(f64::from(rules.rounding_increment)),
        ));
        fields.push((
            "roundingMode",
            self.heap_string_value(&rules.rounding_mode)?,
        ));
        fields.push((
            "roundingPriority",
            self.heap_string_value(&rules.rounding_priority)?,
        ));
        fields.push((
            "trailingZeroDisplay",
            self.heap_string_value(&rules.trailing_zero_display)?,
        ));
        self.create_intl_data_object(fields)
    }

    pub(super) fn eval_intl_plural_rules_supported_locales(
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
        let _matcher = self.plural_string_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            "best fit",
        )?;
        let mut supported = Vec::new();
        for locale in locales {
            if plural_locale_is_supported(&locale) {
                supported.push(self.heap_string_value(&locale)?);
            }
        }
        self.create_array_from_elements(supported)
    }

    fn parse_plural_digit_options(&mut self, options: &Value) -> Result<PluralDigitOptions> {
        let minimum_integer = self.plural_u8_option(options, "minimumIntegerDigits", 1, 21, 1)?;
        let minimum_fraction_value = self.plural_option_value(options, "minimumFractionDigits")?;
        let minimum_fraction =
            self.plural_u8_value(&minimum_fraction_value, "minimumFractionDigits", 0, 100, 0)?;
        let maximum_fraction_value = self.plural_option_value(options, "maximumFractionDigits")?;
        let maximum_fraction = self.plural_u8_value(
            &maximum_fraction_value,
            "maximumFractionDigits",
            minimum_fraction,
            100,
            minimum_fraction.max(3),
        )?;
        let minimum_significant_value =
            self.plural_option_value(options, "minimumSignificantDigits")?;
        let maximum_significant_value =
            self.plural_option_value(options, "maximumSignificantDigits")?;
        let significant_present = !matches!(minimum_significant_value, Value::Undefined)
            || !matches!(maximum_significant_value, Value::Undefined);
        let minimum_significant = significant_present
            .then(|| {
                self.plural_u8_value(
                    &minimum_significant_value,
                    "minimumSignificantDigits",
                    1,
                    21,
                    1,
                )
            })
            .transpose()?;
        let maximum_significant = significant_present
            .then(|| {
                self.plural_u8_value(
                    &maximum_significant_value,
                    "maximumSignificantDigits",
                    minimum_significant.unwrap_or(1),
                    21,
                    21,
                )
            })
            .transpose()?;
        let rounding_increment_value = self.plural_option_value(options, "roundingIncrement")?;
        let rounding_increment =
            self.plural_u16_value(&rounding_increment_value, "roundingIncrement", 1)?;
        if !ROUNDING_INCREMENTS.contains(&rounding_increment) {
            return Err(plural_range_error("roundingIncrement is invalid"));
        }
        let rounding_mode = self.plural_string_option(
            options,
            "roundingMode",
            &[
                "ceil",
                "floor",
                "expand",
                "trunc",
                "halfCeil",
                "halfFloor",
                "halfExpand",
                "halfTrunc",
                "halfEven",
            ],
            "halfExpand",
        )?;
        let rounding_priority = self.plural_string_option(
            options,
            "roundingPriority",
            &["auto", "morePrecision", "lessPrecision"],
            "auto",
        )?;
        let trailing_zero_display = self.plural_string_option(
            options,
            "trailingZeroDisplay",
            &["auto", "stripIfInteger"],
            "auto",
        )?;
        if rounding_increment != 1 && (rounding_priority != "auto" || minimum_significant.is_some())
        {
            return Err(Error::type_error(
                "roundingIncrement is incompatible with digit options",
            ));
        }
        if rounding_increment != 1 && minimum_fraction != maximum_fraction {
            return Err(plural_range_error(
                "roundingIncrement requires equal fraction digit limits",
            ));
        }
        Ok(PluralDigitOptions {
            minimum_integer,
            minimum_fraction,
            maximum_fraction,
            minimum_significant,
            maximum_significant,
            rounding_increment,
            rounding_mode,
            rounding_priority,
            trailing_zero_display,
        })
    }

    fn plural_rules_receiver(&self, this_value: &Value) -> Result<PluralRulesValue> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Intl.PluralRules receiver is invalid"));
        };
        let Some(IntlValue::PluralRules(value)) = self.objects.intl_value(*id)? else {
            return Err(Error::type_error("Intl.PluralRules receiver is invalid"));
        };
        Ok(value.as_ref().clone())
    }

    fn plural_string_option(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
        default: &str,
    ) -> Result<String> {
        let value = self.plural_option_value(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(default.to_owned());
        }
        let text = self.to_string(&value)?;
        if !allowed.contains(&text.as_str()) {
            return Err(plural_range_error(&format!("{name} is invalid")));
        }
        Ok(text)
    }

    fn plural_option_value(&mut self, options: &Value, name: &str) -> Result<Value> {
        if matches!(options, Value::Undefined) {
            return Ok(Value::Undefined);
        }
        self.get_named(options, name)
    }

    fn plural_u8_option(
        &mut self,
        options: &Value,
        name: &str,
        minimum: u8,
        maximum: u8,
        default: u8,
    ) -> Result<u8> {
        let value = self.plural_option_value(options, name)?;
        self.plural_u8_value(&value, name, minimum, maximum, default)
    }

    fn plural_u8_value(
        &mut self,
        value: &Value,
        name: &str,
        minimum: u8,
        maximum: u8,
        default: u8,
    ) -> Result<u8> {
        if matches!(value, Value::Undefined) {
            return Ok(default);
        }
        let number = self.to_number(value)?.floor();
        if !number.is_finite() || number < f64::from(minimum) || number > f64::from(maximum) {
            return Err(plural_range_error(&format!("{name} is out of range")));
        }
        number
            .to_u8()
            .ok_or_else(|| plural_range_error(&format!("{name} is out of range")))
    }

    fn plural_u16_value(&mut self, value: &Value, name: &str, default: u16) -> Result<u16> {
        if matches!(value, Value::Undefined) {
            return Ok(default);
        }
        let number = self.to_number(value)?.floor();
        if !number.is_finite() || number < 1.0 || number > f64::from(u16::MAX) {
            return Err(plural_range_error(&format!("{name} is out of range")));
        }
        number
            .to_u16()
            .ok_or_else(|| plural_range_error(&format!("{name} is out of range")))
    }

    fn install_plural_rules_static_method(&mut self, constructor: NativeFunctionId) -> Result<()> {
        let method = self.create_native_function(
            super::intl_kind(IntlFunctionKind::PluralRulesSupportedLocalesOf),
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

pub(super) fn plural_category(
    locale: &str,
    rule_type: &str,
    number: f64,
    notation: &str,
) -> &'static str {
    if !number.is_finite() {
        return "other";
    }
    let number = number.abs();
    let language = locale_language(locale);
    if rule_type == "ordinal" {
        return ordinal_category(language, number);
    }
    cardinal_category(language, number, notation)
}

fn cardinal_category(language: &str, number: f64, notation: &str) -> &'static str {
    let integer = same_number(number.fract(), 0.0);
    match language {
        "ar" if same_number(number, 0.0) => "zero",
        "ar" | "en" | "pl" if same_number(number, 1.0) => "one",
        "ar" if same_number(number, 2.0) => "two",
        "ar" if integer && (number % 100.0) >= 3.0 && (number % 100.0) <= 10.0 => "few",
        "ar" if integer && (number % 100.0) >= 11.0 => "many",
        "fa" if same_number(number, 0.0) || same_number(number, 1.0) => "one",
        "fr" if notation == "compact" && number >= 1_000_000.0 => "many",
        "fr" if integer && number != 0.0 && number % 1_000_000.0 == 0.0 => "many",
        "fr" if number < 2.0 => "one",
        "pl" if integer
            && matches!(number % 10.0, 2.0..=4.0)
            && !matches!(number % 100.0, 12.0..=14.0) =>
        {
            "few"
        }
        "pl" if integer => "many",
        "sl" if integer && same_number(number % 100.0, 1.0) => "one",
        "sl" if integer && same_number(number % 100.0, 2.0) => "two",
        "sl" if integer && matches!(number % 100.0, 3.0..=4.0) => "few",
        "gv" if integer && same_number(number % 10.0, 1.0) => "one",
        "gv" if integer && same_number(number % 10.0, 2.0) => "two",
        "gv" if integer && matches!(number % 100.0, 0.0 | 20.0 | 40.0 | 60.0 | 80.0) => "few",
        "gv" if !integer => "many",
        _ => "other",
    }
}

fn ordinal_category(language: &str, number: f64) -> &'static str {
    if language != "en" || !same_number(number.fract(), 0.0) {
        return "other";
    }
    let modulo_ten = number % 10.0;
    let modulo_hundred = number % 100.0;
    if same_number(modulo_ten, 1.0) && !same_number(modulo_hundred, 11.0) {
        "one"
    } else if same_number(modulo_ten, 2.0) && !same_number(modulo_hundred, 12.0) {
        "two"
    } else if same_number(modulo_ten, 3.0) && !same_number(modulo_hundred, 13.0) {
        "few"
    } else {
        "other"
    }
}

fn plural_categories(locale: &str, rule_type: &str) -> &'static [&'static str] {
    let language = locale_language(locale);
    if rule_type == "ordinal" && language == "en" {
        return &["one", "two", "few", "other"];
    }
    match language {
        "ar" => &["zero", "one", "two", "few", "many", "other"],
        "en" | "fa" => &["one", "other"],
        "fr" => &["one", "many", "other"],
        "gv" => &["one", "two", "few", "many", "other"],
        "pl" => &["one", "few", "many", "other"],
        "sl" => &["one", "two", "few", "other"],
        _ => &["other"],
    }
}

fn locale_language(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}

const fn plural_locale_is_supported(locale: &str) -> bool {
    !locale.eq_ignore_ascii_case("zxx") && !locale.eq_ignore_ascii_case("und")
}

const fn same_number(left: f64, right: f64) -> bool {
    left.to_bits() == right.to_bits()
}

fn plural_range_error(message: &str) -> Error {
    Error::exception(ErrorName::RangeError, message)
}
