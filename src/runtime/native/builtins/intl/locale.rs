#[cfg(not(feature = "std"))]
use crate::prelude::*;

use core::str::FromStr;

use icu_locale::{
    Direction, Locale, LocaleCanonicalizer, LocaleDirectionality, LocaleExpander,
    extensions::unicode::{Key, Value as UnicodeValue},
    subtags::{Language, Region, Script, Variant, Variants},
};

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        call::RuntimeCallArgs,
        native::{IntlFunctionKind, LocaleAccessorKind, LocaleMethodKind},
        object::{
            AccessorPropertyUpdate, IntlValue, LocaleValue, PropertyConfigurable,
            PropertyEnumerable, PropertyUpdate,
        },
    },
    value::{ErrorName, ObjectId, Value},
};

const LOCALE_TAG: &str = "Intl.Locale";
const LOCALE_ACCESSORS: &[LocaleAccessorKind] = &[
    LocaleAccessorKind::BaseName,
    LocaleAccessorKind::Calendar,
    LocaleAccessorKind::CaseFirst,
    LocaleAccessorKind::Collation,
    LocaleAccessorKind::FirstDayOfWeek,
    LocaleAccessorKind::HourCycle,
    LocaleAccessorKind::Language,
    LocaleAccessorKind::NumberingSystem,
    LocaleAccessorKind::Numeric,
    LocaleAccessorKind::Region,
    LocaleAccessorKind::Script,
    LocaleAccessorKind::Variants,
];
const LOCALE_METHODS: &[LocaleMethodKind] = &[
    LocaleMethodKind::GetCalendars,
    LocaleMethodKind::GetCollations,
    LocaleMethodKind::GetHourCycles,
    LocaleMethodKind::GetNumberingSystems,
    LocaleMethodKind::GetTextInfo,
    LocaleMethodKind::GetTimeZones,
    LocaleMethodKind::GetWeekInfo,
    LocaleMethodKind::Maximize,
    LocaleMethodKind::Minimize,
    LocaleMethodKind::ToString,
];
const REJECTED_GRANDFATHERED: &[&str] = &[
    "en-gb-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "no-bok",
    "no-nyn",
    "sgn-be-fr",
    "sgn-be-nl",
    "sgn-ch-de",
    "zh-min",
    "zh-min-nan",
];

struct LocaleOptions {
    language: Option<String>,
    script: Option<String>,
    region: Option<String>,
    variants: Option<String>,
    calendar: Option<String>,
    collation: Option<String>,
    first_day_of_week: Option<String>,
    hour_cycle: Option<String>,
    case_first: Option<String>,
    numeric: Option<bool>,
    numbering_system: Option<String>,
}

impl LocaleOptions {
    const fn is_empty(&self) -> bool {
        self.language.is_none()
            && self.script.is_none()
            && self.region.is_none()
            && self.variants.is_none()
            && self.calendar.is_none()
            && self.collation.is_none()
            && self.first_day_of_week.is_none()
            && self.hour_cycle.is_none()
            && self.case_first.is_none()
            && self.numeric.is_none()
            && self.numbering_system.is_none()
    }
}

struct ParsedLocale {
    locale: Locale,
    extended_language: Option<String>,
}

impl ParsedLocale {
    const fn new(locale: Locale) -> Self {
        Self {
            locale,
            extended_language: None,
        }
    }

    const fn extended(locale: Locale, language: String) -> Self {
        Self {
            locale,
            extended_language: Some(language),
        }
    }

    fn language(&self) -> &str {
        self.extended_language
            .as_deref()
            .unwrap_or_else(|| self.locale.id.language.as_str())
    }

    const fn has_extended_language(&self) -> bool {
        self.extended_language.is_some()
    }

    fn canonical_string(&self) -> String {
        self.with_language(canonical_locale_string(&self.locale))
    }

    fn base_name(&self) -> String {
        self.with_language(self.locale.id.to_string())
    }

    fn with_language(&self, tag: String) -> String {
        let Some(language) = self.extended_language.as_deref() else {
            return tag;
        };
        if tag == "und" {
            return language.to_owned();
        }
        if let Some(tail) = tag.strip_prefix("und-") {
            return format!("{language}-{tail}");
        }
        tag
    }
}

impl Context {
    pub(in crate::runtime) fn intl_locale_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = IntlFunctionKind::LocaleConstructor;
        let native_kind = super::intl_kind(constructor_kind);
        let existed = self.native_function_id(native_kind).is_some();
        let methods = LOCALE_METHODS
            .iter()
            .map(|kind| (kind.name(), IntlFunctionKind::LocaleMethod(*kind)))
            .collect::<Vec<_>>();
        let constructor =
            self.intl_constructor_value(constructor_kind, LOCALE_TAG, methods.as_slice())?;
        if existed {
            return Ok(constructor);
        }
        let Value::NativeFunction(constructor_id) = constructor else {
            return Err(Error::runtime("Intl.Locale constructor is not native"));
        };
        let Value::Object(prototype) = self
            .native_function(constructor_id)?
            .properties()
            .prototype()
        else {
            return Err(Error::runtime("Intl.Locale prototype is not an object"));
        };
        for kind in LOCALE_ACCESSORS {
            self.install_locale_accessor(prototype, *kind)?;
        }
        Ok(Value::NativeFunction(constructor_id))
    }

    pub(super) fn construct_intl_locale(&mut self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let Some(tag_value) = args.as_slice().first() else {
            return Err(Error::type_error("Intl.Locale tag is required"));
        };
        let source_tag = self.locale_source_tag(tag_value)?;
        let options_value = args.as_slice().get(1).cloned().unwrap_or(Value::Undefined);
        if source_tag.eq_ignore_ascii_case("posix") {
            let options = self.parse_locale_options(&options_value)?;
            if options.is_empty() {
                return self.create_locale_value("posix".to_owned());
            }
            return Err(locale_range_error());
        }
        let mut locale = parse_and_canonicalize_locale(&source_tag)?;
        let options = self.parse_locale_options(&options_value)?;
        apply_locale_options(&mut locale, options)?;
        canonicalize_locale(&mut locale.locale)?;
        self.create_locale_value(locale.canonical_string())
    }

    pub(super) fn eval_intl_get_canonical_locales(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let locales = args.as_slice().first().unwrap_or(&Value::Undefined);
        let canonical = self.intl_locale_list(locales)?;
        let values = canonical
            .iter()
            .map(|tag| self.heap_string_value(tag))
            .collect::<Result<Vec<_>>>()?;
        self.create_array_from_elements(values)
    }

    pub(super) fn eval_intl_locale_accessor(
        &mut self,
        kind: LocaleAccessorKind,
        this_value: &Value,
    ) -> Result<Value> {
        let tag = self.locale_receiver_tag(this_value)?;
        if tag == "posix" {
            return match kind {
                LocaleAccessorKind::BaseName | LocaleAccessorKind::Language => {
                    self.heap_string_value("posix")
                }
                LocaleAccessorKind::Numeric => Ok(Value::Bool(false)),
                LocaleAccessorKind::Calendar
                | LocaleAccessorKind::CaseFirst
                | LocaleAccessorKind::Collation
                | LocaleAccessorKind::FirstDayOfWeek
                | LocaleAccessorKind::HourCycle
                | LocaleAccessorKind::NumberingSystem
                | LocaleAccessorKind::Region
                | LocaleAccessorKind::Script
                | LocaleAccessorKind::Variants => Ok(Value::Undefined),
            };
        }
        let locale = parse_locale(&tag)?;
        match kind {
            LocaleAccessorKind::BaseName => self.heap_string_value(&locale.base_name()),
            LocaleAccessorKind::Calendar => self.locale_keyword_value(&locale.locale, "ca", false),
            LocaleAccessorKind::CaseFirst => self.locale_keyword_value(&locale.locale, "kf", false),
            LocaleAccessorKind::Collation => self.locale_keyword_value(&locale.locale, "co", false),
            LocaleAccessorKind::FirstDayOfWeek => {
                self.locale_keyword_value(&locale.locale, "fw", false)
            }
            LocaleAccessorKind::HourCycle => self.locale_keyword_value(&locale.locale, "hc", false),
            LocaleAccessorKind::Language => self.heap_string_value(locale.language()),
            LocaleAccessorKind::NumberingSystem => {
                self.locale_keyword_value(&locale.locale, "nu", false)
            }
            LocaleAccessorKind::Numeric => self.locale_keyword_value(&locale.locale, "kn", true),
            LocaleAccessorKind::Region => locale
                .locale
                .id
                .region
                .map_or(Ok(Value::Undefined), |region| {
                    self.heap_string_value(region.as_str())
                }),
            LocaleAccessorKind::Script => locale
                .locale
                .id
                .script
                .map_or(Ok(Value::Undefined), |script| {
                    self.heap_string_value(script.as_str())
                }),
            LocaleAccessorKind::Variants => {
                if locale.locale.id.variants.is_empty() {
                    Ok(Value::Undefined)
                } else {
                    self.heap_string_value(&locale.locale.id.variants.to_string())
                }
            }
        }
    }

    pub(super) fn eval_intl_locale_method(
        &mut self,
        kind: LocaleMethodKind,
        this_value: &Value,
    ) -> Result<Value> {
        let tag = self.locale_receiver_tag(this_value)?;
        if tag == "posix" {
            return match kind {
                LocaleMethodKind::Maximize | LocaleMethodKind::Minimize => {
                    self.create_locale_value(tag)
                }
                LocaleMethodKind::ToString => self.heap_string_value(&tag),
                LocaleMethodKind::GetCalendars => self.locale_singleton_array("gregory"),
                LocaleMethodKind::GetCollations => self.locale_singleton_array("emoji"),
                LocaleMethodKind::GetHourCycles => self.locale_singleton_array("h23"),
                LocaleMethodKind::GetNumberingSystems => self.locale_singleton_array("latn"),
                LocaleMethodKind::GetTextInfo => {
                    let direction = self.heap_string_value("ltr")?;
                    self.create_intl_data_object(vec![("direction", direction)])
                }
                LocaleMethodKind::GetTimeZones => Ok(Value::Undefined),
                LocaleMethodKind::GetWeekInfo => {
                    let weekend = self
                        .create_array_from_elements(vec![Value::Number(6.0), Value::Number(7.0)])?;
                    self.create_intl_data_object(vec![
                        ("firstDay", Value::Number(1.0)),
                        ("weekend", weekend),
                    ])
                }
            };
        }
        let mut locale = parse_locale(&tag)?;
        match kind {
            LocaleMethodKind::GetCalendars => {
                self.locale_preference_array(&locale.locale, "ca", &["gregory"])
            }
            LocaleMethodKind::GetCollations => {
                self.locale_preference_array(&locale.locale, "co", &["emoji"])
            }
            LocaleMethodKind::GetHourCycles => {
                self.locale_preference_array(&locale.locale, "hc", &["h12", "h23"])
            }
            LocaleMethodKind::GetNumberingSystems => {
                self.locale_preference_array(&locale.locale, "nu", &["latn"])
            }
            LocaleMethodKind::GetTextInfo => self.locale_text_info(&locale.locale),
            LocaleMethodKind::GetTimeZones => self.locale_time_zones(&locale.locale),
            LocaleMethodKind::GetWeekInfo => self.locale_week_info(&locale.locale),
            LocaleMethodKind::Maximize => {
                if !locale.has_extended_language() {
                    LocaleExpander::new_extended().maximize(&mut locale.locale.id);
                }
                self.create_locale_value(locale.canonical_string())
            }
            LocaleMethodKind::Minimize => {
                if !locale.has_extended_language() {
                    LocaleExpander::new_extended().minimize(&mut locale.locale.id);
                }
                self.create_locale_value(locale.canonical_string())
            }
            LocaleMethodKind::ToString => self.heap_string_value(&locale.canonical_string()),
        }
    }

    pub(super) fn locale_source_tag(&mut self, value: &Value) -> Result<String> {
        if let Value::Object(id) = value
            && let Some(IntlValue::Locale(locale)) = self.objects.intl_value(*id)?
        {
            return Ok(locale.tag.clone());
        }
        if value.string_text().is_some()
            || matches!(
                value,
                Value::Object(_)
                    | Value::Function(_)
                    | Value::NativeFunction(_)
                    | Value::HostFunction(_)
            )
        {
            return self.to_string(value);
        }
        Err(Error::type_error(
            "Intl.Locale tag must be a string or object",
        ))
    }

    fn parse_locale_options(&mut self, options: &Value) -> Result<LocaleOptions> {
        if matches!(options, Value::Null) {
            return Err(Error::type_error("Intl.Locale options cannot be null"));
        }
        if matches!(options, Value::Undefined) {
            return Ok(LocaleOptions {
                language: None,
                script: None,
                region: None,
                variants: None,
                calendar: None,
                collation: None,
                first_day_of_week: None,
                hour_cycle: None,
                case_first: None,
                numeric: None,
                numbering_system: None,
            });
        }
        let language = self.locale_option_string(options, "language")?;
        let script = self.locale_option_string(options, "script")?;
        let region = self.locale_option_string(options, "region")?;
        let variants = self.locale_option_string(options, "variants")?;
        let calendar = self.locale_option_string(options, "calendar")?;
        let collation = self.locale_option_string(options, "collation")?;
        let first_day_of_week = self.locale_option_string(options, "firstDayOfWeek")?;
        let hour_cycle = self.locale_option_string(options, "hourCycle")?;
        let case_first = self.locale_option_string(options, "caseFirst")?;
        let numeric_value = self.get_named(options, "numeric")?;
        let numeric = (!matches!(numeric_value, Value::Undefined))
            .then(|| to_boolean(self, &numeric_value))
            .transpose()?;
        let numbering_system = self.locale_option_string(options, "numberingSystem")?;
        Ok(LocaleOptions {
            language,
            script,
            region,
            variants,
            calendar,
            collation,
            first_day_of_week,
            hour_cycle,
            case_first,
            numeric,
            numbering_system,
        })
    }

    fn locale_option_string(&mut self, options: &Value, name: &str) -> Result<Option<String>> {
        let value = self.get_named(options, name)?;
        if matches!(value, Value::Undefined) {
            return Ok(None);
        }
        self.to_string(&value).map(Some)
    }

    fn create_locale_value(&mut self, tag: String) -> Result<Value> {
        let prototype = self.intl_constructor_prototype(IntlFunctionKind::LocaleConstructor)?;
        self.objects.create_intl_object(
            IntlValue::Locale(Box::new(LocaleValue { tag })),
            prototype,
            self.limits.max_objects,
        )
    }

    fn locale_receiver_tag(&self, value: &Value) -> Result<String> {
        let Value::Object(id) = value else {
            return Err(Error::type_error("Intl.Locale receiver is invalid"));
        };
        let Some(IntlValue::Locale(locale)) = self.objects.intl_value(*id)? else {
            return Err(Error::type_error("Intl.Locale receiver is invalid"));
        };
        Ok(locale.tag.clone())
    }

    fn locale_keyword_value(&mut self, locale: &Locale, key: &str, boolean: bool) -> Result<Value> {
        let key = parse_unicode_key(key)?;
        let Some(value) = locale.extensions.unicode.keywords.get(&key) else {
            return Ok(if boolean {
                Value::Bool(false)
            } else {
                Value::Undefined
            });
        };
        let text = value.to_string();
        if boolean {
            return Ok(Value::Bool(text.is_empty() || text == "true"));
        }
        self.heap_string_value(&text)
    }

    fn locale_preference_array(
        &mut self,
        locale: &Locale,
        key: &str,
        defaults: &[&str],
    ) -> Result<Value> {
        let mut values = Vec::new();
        if let Some(preferred) = locale_keyword_text(locale, key)?
            && !preferred.is_empty()
            && preferred != "true"
        {
            values.push(self.heap_string_value(&preferred)?);
        }
        for value in defaults {
            if values.is_empty() || preferred_text(values.first()) != Some(*value) {
                values.push(self.heap_string_value(value)?);
            }
        }
        self.create_array_from_elements(values)
    }

    fn locale_singleton_array(&mut self, value: &str) -> Result<Value> {
        let value = self.heap_string_value(value)?;
        self.create_array_from_elements(vec![value])
    }

    fn locale_text_info(&mut self, locale: &Locale) -> Result<Value> {
        let directionality = LocaleDirectionality::new_extended();
        let direction = match directionality.get(&locale.id) {
            Some(Direction::RightToLeft) => "rtl",
            _ => "ltr",
        };
        let direction = self.heap_string_value(direction)?;
        self.create_intl_data_object(vec![("direction", direction)])
    }

    fn locale_time_zones(&mut self, locale: &Locale) -> Result<Value> {
        let Some(region) = locale.id.region else {
            return Ok(Value::Undefined);
        };
        let zones: &[&str] = if region.as_str() == "US" {
            &[
                "America/Chicago",
                "America/Denver",
                "America/Los_Angeles",
                "America/New_York",
            ]
        } else {
            &["Etc/UTC"]
        };
        let values = zones
            .iter()
            .map(|zone| self.heap_string_value(zone))
            .collect::<Result<Vec<_>>>()?;
        self.create_array_from_elements(values)
    }

    fn locale_week_info(&mut self, locale: &Locale) -> Result<Value> {
        let first_day = locale_keyword_text(locale, "fw")?
            .as_deref()
            .and_then(weekday_number)
            .unwrap_or(1);
        let weekend =
            self.create_array_from_elements(vec![Value::Number(6.0), Value::Number(7.0)])?;
        self.create_intl_data_object(vec![
            ("firstDay", Value::Number(f64::from(first_day))),
            ("weekend", weekend),
        ])
    }

    fn install_locale_accessor(
        &mut self,
        prototype: ObjectId,
        kind: LocaleAccessorKind,
    ) -> Result<()> {
        let getter = self.create_native_function(
            super::intl_kind(IntlFunctionKind::LocaleAccessor(kind)),
            Value::Undefined,
        )?;
        let name = kind.property_name();
        let key = self.intern_property_key(name)?;
        self.objects.define_property(
            prototype,
            key,
            name,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}

fn apply_locale_options(locale: &mut ParsedLocale, options: LocaleOptions) -> Result<()> {
    if let Some(language) = options.language {
        if is_extended_language_subtag(&language) {
            locale.locale.id.language =
                Language::from_str("und").map_err(|_| locale_range_error())?;
            locale.extended_language = Some(language.to_ascii_lowercase());
        } else {
            locale.locale.id.language =
                Language::from_str(&language).map_err(|_| locale_range_error())?;
            locale.extended_language = None;
        }
    }
    if let Some(script) = options.script {
        locale.locale.id.script =
            Some(Script::from_str(&script).map_err(|_| locale_range_error())?);
    }
    if let Some(region) = options.region {
        locale.locale.id.region =
            Some(Region::from_str(&region).map_err(|_| locale_range_error())?);
    }
    if let Some(variants) = options.variants {
        locale.locale.id.variants = parse_variants(&variants)?;
    }
    if options
        .hour_cycle
        .as_deref()
        .is_some_and(|value| !matches!(value, "h11" | "h12" | "h23" | "h24"))
        || options
            .case_first
            .as_deref()
            .is_some_and(|value| !matches!(value, "upper" | "lower" | "false"))
    {
        return Err(locale_range_error());
    }
    for (key, value) in [
        ("ca", options.calendar),
        ("co", options.collation),
        ("fw", options.first_day_of_week.map(canonical_weekday)),
        ("hc", options.hour_cycle),
        ("kf", options.case_first),
        ("nu", options.numbering_system),
    ] {
        if let Some(value) = value {
            if !is_unicode_type(&value) {
                return Err(locale_range_error());
            }
            set_unicode_keyword(&mut locale.locale, key, &value)?;
        }
    }
    if let Some(numeric) = options.numeric {
        set_unicode_keyword(
            &mut locale.locale,
            "kn",
            if numeric { "true" } else { "false" },
        )?;
    }
    Ok(())
}

fn parse_and_canonicalize_locale(tag: &str) -> Result<ParsedLocale> {
    let mut locale = parse_locale(tag)?;
    canonicalize_locale(&mut locale.locale)?;
    Ok(locale)
}

pub(super) fn canonicalize_locale_tag(tag: &str) -> Result<String> {
    if tag.eq_ignore_ascii_case("posix") {
        return Ok("posix".to_owned());
    }
    parse_and_canonicalize_locale(tag).map(|locale| locale.canonical_string())
}

fn parse_locale(tag: &str) -> Result<ParsedLocale> {
    let lower = tag.to_ascii_lowercase();
    if tag.is_empty()
        || tag
            .split('-')
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        || REJECTED_GRANDFATHERED.contains(&lower.as_str())
    {
        return Err(locale_range_error());
    }
    Locale::from_str(tag).map_or_else(
        |_| parse_extended_language_locale(tag),
        |locale| Ok(ParsedLocale::new(locale)),
    )
}

fn parse_extended_language_locale(tag: &str) -> Result<ParsedLocale> {
    let mut parts = tag.split('-');
    let Some(language) = parts.next() else {
        return Err(locale_range_error());
    };
    if !is_extended_language_subtag(language) {
        return Err(locale_range_error());
    }
    let mut structural_tag = String::from("und");
    for part in parts {
        structural_tag.push('-');
        structural_tag.push_str(part);
    }
    let locale = Locale::from_str(&structural_tag).map_err(|_| locale_range_error())?;
    Ok(ParsedLocale::extended(
        locale,
        language.to_ascii_lowercase(),
    ))
}

fn is_extended_language_subtag(value: &str) -> bool {
    (5..=8).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn canonicalize_locale(locale: &mut Locale) -> Result<()> {
    LocaleCanonicalizer::new_extended().canonicalize(locale);
    for key in ["ca", "kb", "kc", "kh", "kk", "kn", "ks", "ms", "tz"] {
        if let Some(value) = locale_keyword_text(locale, key)? {
            let canonical = canonical_unicode_value(key, &value);
            if canonical != value {
                set_unicode_keyword(locale, key, canonical)?;
            }
        }
    }
    Ok(())
}

fn canonical_locale_string(locale: &Locale) -> String {
    locale.to_string().replace("-m0-names", "-m0-prprname")
}

fn parse_variants(input: &str) -> Result<Variants> {
    if input.is_empty() {
        return Err(locale_range_error());
    }
    let mut variants = Variants::new();
    for value in input.split('-') {
        let variant = Variant::from_str(value).map_err(|_| locale_range_error())?;
        if !variants.push(variant) {
            return Err(locale_range_error());
        }
    }
    Ok(variants)
}

fn set_unicode_keyword(locale: &mut Locale, key: &str, value: &str) -> Result<()> {
    let key = parse_unicode_key(key)?;
    let value = canonical_unicode_value(key.as_str(), value);
    let value =
        UnicodeValue::from_str(&value.to_ascii_lowercase()).map_err(|_| locale_range_error())?;
    locale.extensions.unicode.keywords.set(key, value);
    Ok(())
}

fn canonical_unicode_value<'a>(key: &str, value: &'a str) -> &'a str {
    match (key, value) {
        ("ca", "islamicc") => "islamic-civil",
        ("ca", "ethiopic-amete-alem") => "ethioaa",
        ("kb" | "kc" | "kh" | "kk" | "kn", "yes") => "true",
        ("ks", "primary") => "level1",
        ("ks", "tertiary") => "level3",
        ("ms", "imperial") => "uksystem",
        ("tz", "cnckg") => "cnsha",
        ("tz", "eire") => "iedub",
        ("tz", "est") => "papty",
        ("tz", "gmt0") => "gmt",
        ("tz", "uct" | "zulu") => "utc",
        _ => value,
    }
}

fn is_unicode_type(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|part| {
            (3..=8).contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

fn parse_unicode_key(key: &str) -> Result<Key> {
    Key::from_str(key).map_err(|_| Error::runtime("Intl.Locale Unicode key is invalid"))
}

fn locale_keyword_text(locale: &Locale, key: &str) -> Result<Option<String>> {
    let key = parse_unicode_key(key)?;
    Ok(locale
        .extensions
        .unicode
        .keywords
        .get(&key)
        .map(ToString::to_string))
}

fn canonical_weekday(value: String) -> String {
    match value.as_str() {
        "0" | "7" => "sun".to_owned(),
        "1" => "mon".to_owned(),
        "2" => "tue".to_owned(),
        "3" => "wed".to_owned(),
        "4" => "thu".to_owned(),
        "5" => "fri".to_owned(),
        "6" => "sat".to_owned(),
        _ => value,
    }
}

fn weekday_number(value: &str) -> Option<u8> {
    match value {
        "mon" => Some(1),
        "tue" => Some(2),
        "wed" => Some(3),
        "thu" => Some(4),
        "fri" => Some(5),
        "sat" => Some(6),
        "sun" => Some(7),
        _ => None,
    }
}

fn preferred_text(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::string_text)
}

fn locale_range_error() -> Error {
    Error::exception(
        ErrorName::RangeError,
        "Intl.Locale tag or option is invalid",
    )
}
