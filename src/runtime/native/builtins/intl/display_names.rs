use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::IntlFunctionKind,
        object::{DisplayNamesValue, IntlValue},
    },
    value::{ErrorName, Value},
};

const DISPLAY_NAMES_TAG: &str = "Intl.DisplayNames";
const DEFAULT_LOCALE: &str = "en-US";
pub(super) const SUPPORTED_CURRENCIES: &[&str] = &[
    "AUD", "BRL", "CAD", "CHF", "CNY", "EUR", "GBP", "HKD", "INR", "JPY", "KRW", "MXN", "NZD",
    "SEK", "SGD", "USD", "ZAR",
];
const DATE_TIME_FIELDS: &[&str] = &[
    "era",
    "year",
    "quarter",
    "month",
    "weekOfYear",
    "weekday",
    "day",
    "dayPeriod",
    "hour",
    "minute",
    "second",
    "timeZoneName",
];

impl Context {
    pub(in crate::runtime) fn intl_display_names_constructor_value(&mut self) -> Result<Value> {
        self.intl_constructor_value(
            IntlFunctionKind::DisplayNamesConstructor,
            DISPLAY_NAMES_TAG,
            &[
                ("of", IntlFunctionKind::DisplayNamesOf),
                (
                    "resolvedOptions",
                    IntlFunctionKind::DisplayNamesResolvedOptions,
                ),
            ],
        )
    }

    pub(super) fn construct_intl_display_names(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let requested = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        let locales = self.intl_locale_list(&requested)?;
        let options = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if !is_object_value(&options) {
            return Err(Error::type_error(
                "Intl.DisplayNames options must be an object",
            ));
        }
        let _matcher = self.display_names_option(
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
            Some("best fit"),
        )?;
        let style = self
            .display_names_option(
                &options,
                "style",
                &["long", "short", "narrow"],
                Some("long"),
            )?
            .ok_or_else(|| Error::runtime("DisplayNames style default is missing"))?;
        let display_type = self
            .display_names_option(
                &options,
                "type",
                &[
                    "language",
                    "region",
                    "script",
                    "currency",
                    "calendar",
                    "dateTimeField",
                ],
                None,
            )?
            .ok_or_else(|| Error::type_error("Intl.DisplayNames type is required"))?;
        let missing_code_behavior = self
            .display_names_option(&options, "fallback", &["code", "none"], Some("code"))?
            .ok_or_else(|| Error::runtime("DisplayNames missing-code default is absent"))?;
        let language_display = if display_type == "language" {
            Some(
                self.display_names_option(
                    &options,
                    "languageDisplay",
                    &["dialect", "standard"],
                    Some("dialect"),
                )?
                .ok_or_else(|| Error::runtime("DisplayNames languageDisplay default is missing"))?,
            )
        } else {
            None
        };
        let locale = locales
            .into_iter()
            .find(|locale| !locale.eq_ignore_ascii_case("zxx"))
            .unwrap_or_else(|| DEFAULT_LOCALE.to_owned());
        let prototype =
            self.intl_constructor_prototype(IntlFunctionKind::DisplayNamesConstructor)?;
        self.objects.create_intl_object(
            IntlValue::DisplayNames(Box::new(DisplayNamesValue {
                locale,
                style,
                display_type,
                missing_code_behavior,
                language_display,
            })),
            prototype,
            self.limits.max_objects,
        )
    }

    pub(super) fn eval_intl_display_names_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.display_names_receiver(this_value)?;
        let code = self.to_string(args.as_slice().first().unwrap_or(&Value::Undefined))?;
        let normalized = normalize_display_name_code(&formatter.display_type, &code)?;
        if formatter.display_type == "currency"
            && formatter.missing_code_behavior == "none"
            && !SUPPORTED_CURRENCIES.contains(&normalized.as_str())
        {
            return Ok(Value::Undefined);
        }
        self.heap_string_value(&normalized)
    }

    pub(super) fn eval_intl_display_names_resolved_options(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        let formatter = self.display_names_receiver(this_value)?;
        let locale = self.heap_string_value(&formatter.locale)?;
        let style = self.heap_string_value(&formatter.style)?;
        let display_type = self.heap_string_value(&formatter.display_type)?;
        let missing_code_behavior = self.heap_string_value(&formatter.missing_code_behavior)?;
        let mut fields = vec![
            ("locale", locale),
            ("style", style),
            ("type", display_type),
            ("fallback", missing_code_behavior),
        ];
        if let Some(language_display) = formatter.language_display {
            fields.push((
                "languageDisplay",
                self.heap_string_value(&language_display)?,
            ));
        }
        self.create_intl_data_object(fields)
    }

    fn display_names_receiver(&self, this_value: &Value) -> Result<DisplayNamesValue> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error("Intl.DisplayNames receiver is invalid"));
        };
        let Some(IntlValue::DisplayNames(value)) = self.objects.intl_value(*id)? else {
            return Err(Error::type_error("Intl.DisplayNames receiver is invalid"));
        };
        Ok(value.as_ref().clone())
    }

    fn display_names_option(
        &mut self,
        options: &Value,
        name: &str,
        allowed: &[&str],
        default: Option<&str>,
    ) -> Result<Option<String>> {
        let value = self.get_named(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(default.map(str::to_owned));
        }
        let text = self.to_string(&value)?;
        if !allowed.contains(&text.as_str()) {
            return Err(display_names_range_error(name));
        }
        Ok(Some(text))
    }
}

const fn is_object_value(value: &Value) -> bool {
    matches!(
        value,
        Value::Object(_) | Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_)
    )
}

fn normalize_display_name_code(display_type: &str, code: &str) -> Result<String> {
    match display_type {
        "language" if valid_language_code(code) => Ok(code.to_owned()),
        "region" if valid_region_code(code) => Ok(code.to_ascii_uppercase()),
        "script" if valid_script_code(code) => {
            let mut normalized = code.to_ascii_lowercase();
            if let Some(first) = normalized.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            Ok(normalized)
        }
        "currency" if valid_currency_code(code) => Ok(code.to_ascii_uppercase()),
        "calendar" if valid_calendar_code(code) => Ok(code.to_ascii_lowercase()),
        "dateTimeField" if DATE_TIME_FIELDS.contains(&code) => Ok(code.to_owned()),
        _ => Err(display_names_range_error("code")),
    }
}

fn valid_language_code(code: &str) -> bool {
    let mut parts = code.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    let language_length = language.len();
    if !matches!(language_length, 2 | 3 | 5..=8)
        || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
        || language.eq_ignore_ascii_case("root")
    {
        return false;
    }
    let mut script_seen = false;
    let mut region_seen = false;
    let mut variants = Vec::new();
    for part in parts {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return false;
        }
        if !script_seen
            && !region_seen
            && part.len() == 4
            && part.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            script_seen = true;
            continue;
        }
        if !region_seen && valid_region_code(part) {
            region_seen = true;
            continue;
        }
        let variant = (5..=8).contains(&part.len())
            || (part.len() == 4 && part.as_bytes().first().is_some_and(u8::is_ascii_digit));
        let normalized = part.to_ascii_lowercase();
        if !variant || variants.contains(&normalized) {
            return false;
        }
        variants.push(normalized);
    }
    true
}

fn valid_region_code(code: &str) -> bool {
    (code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_alphabetic()))
        || (code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_digit()))
}

fn valid_script_code(code: &str) -> bool {
    code.len() == 4 && code.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn valid_currency_code(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn valid_calendar_code(code: &str) -> bool {
    let mut saw_part = false;
    for part in code.split('-') {
        saw_part = true;
        if !(3..=8).contains(&part.len()) || !part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return false;
        }
    }
    saw_part
}

fn display_names_range_error(name: &str) -> Error {
    Error::exception(
        ErrorName::RangeError,
        format!("Intl.DisplayNames {name} has an unsupported value"),
    )
}
